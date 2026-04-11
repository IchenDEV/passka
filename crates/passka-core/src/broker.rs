use crate::oauth;
use crate::store::keychain::KeychainStore;
use crate::types::{
    AccessContext, AccessLease, AuditEvent, AuditEventKind, AuditOutcome, AuthorizationSession,
    BrokerPolicy, HttpProxyResponse, HttpRequestSpec, Principal, PrincipalKind, ProviderAccount,
    ProviderSecret, RegisterProviderAccount, ResourceGrant,
};
use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::header::{HeaderMap as ReqwestHeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const BROKER_SERVICE_NAME: &str = "passka-broker";
#[cfg(test)]
const DEFAULT_LEASE_SECONDS: i64 = 300;
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);
const API_KEY_PLACEHOLDERS: &[&str] = &[
    "{{PASSKA_API_KEY}}",
    "$PASSKA_API_KEY",
    "PASSKA_API_KEY",
    "{{PASSKA_TOKEN}}",
    "$PASSKA_TOKEN",
    "PASSKA_TOKEN",
];

#[derive(Debug, Clone)]
struct ProxyCredential {
    alias: String,
    lease: AccessLease,
    account: ProviderAccount,
    secret: ProviderSecret,
    inject_auth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BrokerState {
    principals: Vec<Principal>,
    accounts: Vec<ProviderAccount>,
    grants: Vec<ResourceGrant>,
    policies: Vec<BrokerPolicy>,
    leases: Vec<AccessLease>,
    audit_events: Vec<AuditEvent>,
}

#[derive(Clone)]
pub struct Broker {
    state_path: PathBuf,
}

impl Broker {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("cannot determine config directory")?
            .join("passka")
            .join("broker");
        Self::from_dir(config_dir)
    }

    fn from_dir(config_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&config_dir)?;
        let broker = Self {
            state_path: config_dir.join("state.json"),
        };
        broker.bootstrap()?;
        Ok(broker)
    }

    pub fn list_principals(&self) -> Result<Vec<Principal>> {
        Ok(self.load_state()?.principals)
    }

    pub fn add_principal(
        &self,
        name: &str,
        kind: PrincipalKind,
        description: &str,
    ) -> Result<Principal> {
        let mut state = self.load_state()?;
        if state
            .principals
            .iter()
            .any(|principal| principal.name == name)
        {
            anyhow::bail!("principal '{name}' already exists");
        }
        let now = now();
        let principal = Principal {
            id: new_id("principal"),
            name: name.to_string(),
            kind,
            description: description.to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        state.principals.push(principal.clone());
        self.append_audit(
            &mut state,
            principal.id.as_str(),
            AuditEventKind::PrincipalCreated,
            AuditOutcome::Success,
            format!("principal:{}", principal.name),
            format!("created {} principal", principal.kind),
        );
        self.save_state(&state)?;
        Ok(principal)
    }

    pub fn list_accounts(&self) -> Result<Vec<ProviderAccount>> {
        Ok(self.load_state()?.accounts)
    }

    pub fn get_account(&self, account_id: &str) -> Result<ProviderAccount> {
        self.load_state()?
            .accounts
            .into_iter()
            .find(|account| account.id == account_id)
            .ok_or_else(|| anyhow::anyhow!("account '{account_id}' not found"))
    }

    pub fn register_provider_account(
        &self,
        request: RegisterProviderAccount,
    ) -> Result<ProviderAccount> {
        let mut state = self.load_state()?;
        if state
            .accounts
            .iter()
            .any(|account| account.name == request.name)
        {
            anyhow::bail!("account '{}' already exists", request.name);
        }

        let now = now();
        let account = ProviderAccount {
            id: new_id("account"),
            name: request.name,
            provider: request.provider,
            auth_method: request.secret.auth_method(),
            base_url: request.base_url,
            description: request.description,
            scopes: request.scopes,
            created_at: now.clone(),
            updated_at: now,
        };

        self.save_secret(&account.id, &request.secret)?;
        state.accounts.push(account.clone());
        self.append_audit(
            &mut state,
            "principal:local-human",
            AuditEventKind::AccountRegistered,
            AuditOutcome::Success,
            format!("account:{}", account.id),
            format!(
                "registered provider account '{}' with auth {}",
                account.name, account.auth_method
            ),
        );
        self.save_state(&state)?;
        Ok(account)
    }

    pub fn remove_account(&self, account_id: &str) -> Result<()> {
        let mut state = self.load_state()?;
        let before = state.accounts.len();
        state.accounts.retain(|account| account.id != account_id);
        if state.accounts.len() == before {
            anyhow::bail!("account '{account_id}' not found");
        }
        state.grants.retain(|grant| grant.account_id != account_id);
        for policy in &mut state.policies {
            policy.grant_ids.retain(|grant_id| {
                state
                    .grants
                    .iter()
                    .any(|grant| grant.id.as_str() == grant_id.as_str())
            });
        }
        state.policies.retain(|policy| !policy.grant_ids.is_empty());
        let _ = KeychainStore::delete(BROKER_SERVICE_NAME, &secret_key(account_id));
        self.append_audit(
            &mut state,
            "principal:local-human",
            AuditEventKind::AccountRemoved,
            AuditOutcome::Success,
            format!("account:{account_id}"),
            "removed provider account".into(),
        );
        self.save_state(&state)?;
        Ok(())
    }

    pub fn list_policies(&self) -> Result<Vec<BrokerPolicy>> {
        Ok(self.load_state()?.policies)
    }

    pub fn allow_policy(
        &self,
        principal_id: &str,
        account_id: &str,
        resource: &str,
        actions: Vec<String>,
        environments: Vec<String>,
        max_lease_seconds: i64,
        allow_secret_reveal: bool,
        description: &str,
    ) -> Result<BrokerPolicy> {
        let mut state = self.load_state()?;
        if !state
            .principals
            .iter()
            .any(|principal| principal.id == principal_id)
        {
            anyhow::bail!("principal '{principal_id}' not found");
        }
        if !state
            .accounts
            .iter()
            .any(|account| account.id == account_id)
        {
            anyhow::bail!("account '{account_id}' not found");
        }

        let grant = ResourceGrant {
            id: new_id("grant"),
            account_id: account_id.to_string(),
            resource: resource.to_string(),
            actions,
            description: description.to_string(),
            created_at: now(),
        };
        let now = now();
        let policy = BrokerPolicy {
            id: new_id("policy"),
            principal_id: principal_id.to_string(),
            grant_ids: vec![grant.id.clone()],
            environments,
            allow_secret_reveal,
            max_lease_seconds: max_lease_seconds.max(30),
            created_at: now.clone(),
            updated_at: now,
        };

        state.grants.push(grant);
        state.policies.push(policy.clone());
        self.append_audit(
            &mut state,
            principal_id,
            AuditEventKind::PolicyCreated,
            AuditOutcome::Success,
            format!("account:{account_id}"),
            format!("policy '{}' created", policy.id),
        );
        self.save_state(&state)?;
        Ok(policy)
    }

    pub fn list_audit_events(&self, limit: Option<usize>) -> Result<Vec<AuditEvent>> {
        let mut events = self.load_state()?.audit_events;
        events.reverse();
        if let Some(limit) = limit {
            events.truncate(limit);
        }
        Ok(events)
    }

    pub fn start_authorization(&self, account_id: &str) -> Result<AuthorizationSession> {
        let mut state = self.load_state()?;
        let account = state
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("account '{account_id}' not found"))?;
        let secret = self.load_secret(account_id)?;
        let ProviderSecret::OAuth(secret) = secret else {
            anyhow::bail!("account '{}' is not OAuth-backed", account.name);
        };

        let mut url = url::Url::parse(&secret.authorize_url).context("invalid authorize_url")?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &secret.client_id)
            .append_pair("redirect_uri", &secret.redirect_uri);
        if !secret.scopes.is_empty() {
            url.query_pairs_mut()
                .append_pair("scope", &secret.scopes.join(" "));
        }

        self.append_audit(
            &mut state,
            "principal:local-human",
            AuditEventKind::AuthorizationStarted,
            AuditOutcome::Success,
            format!("account:{account_id}"),
            "started OAuth authorization".into(),
        );
        self.save_state(&state)?;
        Ok(AuthorizationSession {
            account_id: account_id.to_string(),
            authorization_url: url.to_string(),
        })
    }

    pub fn complete_authorization(&self, account_id: &str, code: &str) -> Result<()> {
        let secret = self.load_secret(account_id)?;
        let ProviderSecret::OAuth(secret) = secret else {
            anyhow::bail!("account '{account_id}' is not OAuth-backed");
        };
        let runtime = tokio::runtime::Runtime::new()?;
        let refreshed = runtime.block_on(oauth::exchange_code(&secret, code))?;
        self.save_secret(account_id, &ProviderSecret::OAuth(refreshed))?;

        let mut state = self.load_state()?;
        self.touch_account(&mut state, account_id)?;
        self.append_audit(
            &mut state,
            "principal:local-human",
            AuditEventKind::AuthorizationCompleted,
            AuditOutcome::Success,
            format!("account:{account_id}"),
            "completed OAuth authorization".into(),
        );
        self.save_state(&state)?;
        Ok(())
    }

    pub fn refresh_account(&self, account_id: &str) -> Result<()> {
        let secret = self.load_secret(account_id)?;
        let ProviderSecret::OAuth(secret) = secret else {
            anyhow::bail!("account '{account_id}' is not OAuth-backed");
        };
        let runtime = tokio::runtime::Runtime::new()?;
        let refreshed = runtime.block_on(oauth::refresh_token(&secret))?;
        self.save_secret(account_id, &ProviderSecret::OAuth(refreshed))?;

        let mut state = self.load_state()?;
        self.touch_account(&mut state, account_id)?;
        self.append_audit(
            &mut state,
            "principal:local-human",
            AuditEventKind::TokenRefreshed,
            AuditOutcome::Success,
            format!("account:{account_id}"),
            "refreshed OAuth access token".into(),
        );
        self.save_state(&state)?;
        Ok(())
    }

    pub fn reveal_sensitive_field(
        &self,
        actor_principal_id: &str,
        account_id: &str,
        field: &str,
    ) -> Result<String> {
        let mut state = self.load_state()?;
        self.ensure_sensitive_access(
            &mut state,
            actor_principal_id,
            format!("account:{account_id}:{field}"),
        )?;

        let secret = self.load_secret(account_id)?;
        let value = secret.reveal_field(field).ok_or_else(|| {
            anyhow::anyhow!("field '{field}' not found on account '{account_id}'")
        })?;
        self.append_audit(
            &mut state,
            actor_principal_id,
            AuditEventKind::SecretViewed,
            AuditOutcome::Success,
            format!("account:{account_id}:{field}"),
            "sensitive field revealed".into(),
        );
        self.save_state(&state)?;
        Ok(value)
    }

    pub fn request_access(
        &self,
        principal_id: &str,
        resource: &str,
        action: &str,
        context: AccessContext,
    ) -> Result<AccessLease> {
        let mut state = self.load_state()?;
        if !state
            .principals
            .iter()
            .any(|principal| principal.id == principal_id)
        {
            anyhow::bail!("principal '{principal_id}' not found");
        }

        let decision = state
            .policies
            .iter()
            .filter(|policy| policy.principal_id == principal_id)
            .find_map(|policy| {
                if !policy.environments.is_empty()
                    && !context.environment.is_empty()
                    && !policy
                        .environments
                        .iter()
                        .any(|env| env == &context.environment)
                {
                    return None;
                }
                policy.grant_ids.iter().find_map(|grant_id| {
                    let grant = state.grants.iter().find(|grant| &grant.id == grant_id)?;
                    if grant.actions.iter().any(|allowed| allowed == action)
                        && resource_matches(&grant.resource, resource)
                    {
                        Some((policy.clone(), grant.clone()))
                    } else {
                        None
                    }
                })
            });

        let Some((policy, grant)) = decision else {
            self.append_audit(
                &mut state,
                principal_id,
                AuditEventKind::AccessDenied,
                AuditOutcome::Denied,
                resource.to_string(),
                format!(
                    "denied action '{action}' in environment '{}'",
                    context.environment
                ),
            );
            self.save_state(&state)?;
            anyhow::bail!(
                "no policy allows principal '{principal_id}' to perform '{action}' on '{resource}'"
            );
        };

        let now = now();
        let expires_at =
            (Utc::now() + chrono::Duration::seconds(policy.max_lease_seconds.max(30))).to_rfc3339();
        let lease = AccessLease {
            id: new_id("lease"),
            principal_id: principal_id.to_string(),
            account_id: grant.account_id.clone(),
            grant_id: grant.id.clone(),
            resource: resource.to_string(),
            action: action.to_string(),
            expires_at,
            created_at: now,
            context,
        };
        state.leases.push(lease.clone());
        self.append_audit(
            &mut state,
            principal_id,
            AuditEventKind::AccessGranted,
            AuditOutcome::Success,
            resource.to_string(),
            format!("lease '{}' created", lease.id),
        );
        self.save_state(&state)?;
        Ok(lease)
    }

    pub fn proxy_http(&self, lease_id: &str, spec: HttpRequestSpec) -> Result<HttpProxyResponse> {
        self.proxy_http_with_leases(lease_id, HashMap::new(), spec)
    }

    pub fn proxy_http_with_leases(
        &self,
        lease_id: &str,
        extra_leases: HashMap<String, String>,
        spec: HttpRequestSpec,
    ) -> Result<HttpProxyResponse> {
        let (lease, account) = self.active_lease_account(lease_id)?;
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            let url = build_proxy_url(&account, &spec.path)?;
            let credentials = self.proxy_credentials(&lease, &extra_leases).await?;
            self.proxy_request_async(
                &credentials,
                &spec.method,
                &url,
                spec.headers,
                spec.body.into_bytes(),
            )
            .await
        })
    }

    pub fn proxy_forward_http(
        &self,
        lease_id: &str,
        method: &str,
        target_url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<HttpProxyResponse> {
        self.proxy_forward_http_with_leases(
            lease_id,
            HashMap::new(),
            method,
            target_url,
            headers,
            body,
        )
    }

    pub fn proxy_forward_http_with_leases(
        &self,
        lease_id: &str,
        extra_leases: HashMap<String, String>,
        method: &str,
        target_url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<HttpProxyResponse> {
        let (lease, _) = self.active_lease_account(lease_id)?;
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            validate_proxy_url(target_url)?;
            let credentials = self.proxy_credentials(&lease, &extra_leases).await?;
            self.proxy_request_async(&credentials, method, target_url, headers, body)
                .await
        })
    }

    fn active_lease_account(&self, lease_id: &str) -> Result<(AccessLease, ProviderAccount)> {
        let state = self.load_state()?;
        let lease = state
            .leases
            .iter()
            .find(|lease| lease.id == lease_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("lease '{lease_id}' not found"))?;
        let expires_at = chrono::DateTime::parse_from_rfc3339(&lease.expires_at)
            .context("invalid lease expiry")?;
        if Utc::now() >= expires_at {
            anyhow::bail!("lease '{lease_id}' has expired");
        }

        let account = state
            .accounts
            .iter()
            .find(|account| account.id == lease.account_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("account '{}' not found", lease.account_id))?;

        Ok((lease, account))
    }

    async fn proxy_request_async(
        &self,
        credentials: &[ProxyCredential],
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<HttpProxyResponse> {
        let primary = credentials
            .first()
            .ok_or_else(|| anyhow::anyhow!("proxy request has no primary lease"))?;
        let method_label = method.to_string();
        let method: reqwest::Method = method.parse().context("invalid HTTP method")?;
        let headers = materialize_forward_headers(headers, credentials)?;
        let body = materialize_forward_body(body, credentials)?;
        let client = reqwest::Client::new();
        let mut request = client.request(method, url).headers(headers);
        if !body.is_empty() {
            request = request.body(body);
        }

        let response = request.send().await.context("proxy request failed")?;
        let status = response.status().as_u16();
        let mut headers = HashMap::new();
        for (name, value) in response.headers() {
            headers.insert(
                name.to_string(),
                value.to_str().unwrap_or_default().to_string(),
            );
        }
        let body = response
            .text()
            .await
            .context("failed to read proxy response")?;

        let mut state = self.load_state()?;
        self.append_audit(
            &mut state,
            &primary.lease.principal_id,
            AuditEventKind::ProxyRequest,
            AuditOutcome::Success,
            primary.lease.resource.clone(),
            format!("proxied {} {}", method_label, url),
        );
        self.save_state(&state)?;

        Ok(HttpProxyResponse {
            status,
            headers,
            body,
        })
    }

    async fn proxy_credentials(
        &self,
        primary_lease: &AccessLease,
        extra_leases: &HashMap<String, String>,
    ) -> Result<Vec<ProxyCredential>> {
        let (_, primary_account) = self.active_lease_account(&primary_lease.id)?;
        let mut credentials = vec![ProxyCredential {
            alias: "DEFAULT".into(),
            lease: primary_lease.clone(),
            account: primary_account,
            secret: self.load_proxy_secret(primary_lease).await?,
            inject_auth: true,
        }];

        for (alias, lease_id) in extra_leases {
            let alias = normalize_placeholder_alias(alias)?;
            if alias == "DEFAULT" {
                anyhow::bail!("extra lease alias 'default' is reserved");
            }
            let (lease, account) = self.active_lease_account(lease_id)?;
            if lease.principal_id != primary_lease.principal_id {
                anyhow::bail!(
                    "extra lease '{}' belongs to a different principal",
                    lease.id
                );
            }
            let secret = self.load_proxy_secret(&lease).await?;
            credentials.push(ProxyCredential {
                alias,
                lease,
                account,
                secret,
                inject_auth: false,
            });
        }

        Ok(credentials)
    }

    async fn load_proxy_secret(&self, lease: &AccessLease) -> Result<ProviderSecret> {
        let mut secret = self.load_secret(&lease.account_id)?;
        if let ProviderSecret::OAuth(oauth_secret) = &secret {
            if oauth::needs_refresh(oauth_secret)? {
                let refreshed = oauth::refresh_token(oauth_secret).await?;
                secret = ProviderSecret::OAuth(refreshed.clone());
                self.save_secret(&lease.account_id, &secret)?;

                let mut state = self.load_state()?;
                self.append_audit(
                    &mut state,
                    &lease.principal_id,
                    AuditEventKind::TokenRefreshed,
                    AuditOutcome::Success,
                    format!("account:{}", lease.account_id),
                    "refreshed OAuth token during proxy request".into(),
                );
                self.save_state(&state)?;
            }
        }
        Ok(secret)
    }

    fn bootstrap(&self) -> Result<()> {
        if !self.state_path.exists() {
            self.save_state(&BrokerState::default())?;
        }
        self.ensure_default_principals()?;
        Ok(())
    }

    fn ensure_default_principals(&self) -> Result<()> {
        let mut state = self.load_state()?;
        let mut changed = false;
        for (id, name, kind, description) in [
            (
                "principal:local-human",
                "Local Human",
                PrincipalKind::Human,
                "Default local operator",
            ),
            (
                "principal:local-agent",
                "Local Agent",
                PrincipalKind::Agent,
                "Default local AI agent principal",
            ),
        ] {
            if !state.principals.iter().any(|principal| principal.id == id) {
                let timestamp = now();
                state.principals.push(Principal {
                    id: id.to_string(),
                    name: name.to_string(),
                    kind,
                    description: description.to_string(),
                    created_at: timestamp.clone(),
                    updated_at: timestamp,
                });
                changed = true;
            }
        }
        if changed {
            self.save_state(&state)?;
        }
        Ok(())
    }

    fn load_state(&self) -> Result<BrokerState> {
        let content = fs::read_to_string(&self.state_path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn save_state(&self, state: &BrokerState) -> Result<()> {
        fs::write(&self.state_path, serde_json::to_string_pretty(state)?)?;
        Ok(())
    }

    fn save_secret(&self, account_id: &str, secret: &ProviderSecret) -> Result<()> {
        KeychainStore::set_json(BROKER_SERVICE_NAME, &secret_key(account_id), secret)
    }

    fn load_secret(&self, account_id: &str) -> Result<ProviderSecret> {
        KeychainStore::get_json(BROKER_SERVICE_NAME, &secret_key(account_id))
    }

    fn ensure_sensitive_access(
        &self,
        state: &mut BrokerState,
        actor_principal_id: &str,
        resource: String,
    ) -> Result<()> {
        let principal = state
            .principals
            .iter()
            .find(|principal| principal.id == actor_principal_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("principal '{actor_principal_id}' not found"))?;
        if principal.kind != PrincipalKind::Human {
            self.append_audit(
                state,
                actor_principal_id,
                AuditEventKind::SecretRevealDenied,
                AuditOutcome::Denied,
                resource,
                "only human principals can reveal sensitive values".into(),
            );
            self.save_state(state)?;
            anyhow::bail!("principal '{actor_principal_id}' cannot reveal sensitive fields");
        }

        let allowed = state
            .policies
            .iter()
            .any(|policy| policy.principal_id == actor_principal_id && policy.allow_secret_reveal);
        let is_default_human = actor_principal_id == "principal:local-human";
        if !(allowed || is_default_human) {
            self.append_audit(
                state,
                actor_principal_id,
                AuditEventKind::SecretRevealDenied,
                AuditOutcome::Denied,
                resource,
                "principal has no secret reveal permission".into(),
            );
            self.save_state(state)?;
            anyhow::bail!("principal '{actor_principal_id}' is not allowed to reveal secrets");
        }

        Ok(())
    }

    fn touch_account(&self, state: &mut BrokerState, account_id: &str) -> Result<()> {
        let account = state
            .accounts
            .iter_mut()
            .find(|account| account.id == account_id)
            .ok_or_else(|| anyhow::anyhow!("account '{account_id}' not found"))?;
        account.updated_at = now();
        Ok(())
    }

    fn append_audit(
        &self,
        state: &mut BrokerState,
        actor_principal_id: &str,
        kind: AuditEventKind,
        outcome: AuditOutcome,
        resource: String,
        detail: String,
    ) {
        state.audit_events.push(AuditEvent {
            id: new_id("audit"),
            timestamp: now(),
            actor_principal_id: actor_principal_id.to_string(),
            kind,
            outcome,
            resource,
            detail,
        });
    }
}

fn materialize_forward_headers(
    headers: HashMap<String, String>,
    credentials: &[ProxyCredential],
) -> Result<ReqwestHeaderMap> {
    let mut materialized = ReqwestHeaderMap::new();
    for (name, value) in headers {
        if is_hop_by_hop_header(&name) || is_passka_control_header(&name) {
            continue;
        }
        let header_name: HeaderName = name
            .parse()
            .with_context(|| format!("invalid request header '{name}'"))?;
        let header_value = replace_secret_placeholders(&value, credentials)?;
        let header_value: HeaderValue = header_value
            .parse()
            .with_context(|| format!("invalid request header value for '{name}'"))?;
        materialized.insert(header_name, header_value);
    }

    let Some(primary) = credentials.iter().find(|credential| credential.inject_auth) else {
        anyhow::bail!("proxy request has no credential selected for auth injection");
    };
    match &primary.secret {
        ProviderSecret::Opaque(_) => anyhow::bail!("opaque secrets cannot be proxied over HTTP"),
        ProviderSecret::ApiKey(secret) => {
            let name: HeaderName = secret
                .header_name
                .parse()
                .with_context(|| format!("invalid auth header '{}'", secret.header_name))?;
            let value = if secret.header_prefix.is_empty() {
                secret.api_key.clone()
            } else {
                format!("{} {}", secret.header_prefix, secret.api_key)
            };
            let value: HeaderValue = value.parse().with_context(|| {
                format!("invalid auth header value for '{}'", secret.header_name)
            })?;
            materialized.insert(name, value);
        }
        ProviderSecret::OAuth(secret) => {
            if secret.access_token.is_empty() {
                anyhow::bail!("OAuth account has no access token; run authorization first");
            }
            materialized.insert(
                reqwest::header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", secret.access_token))
                    .context("invalid OAuth bearer token")?,
            );
        }
    }

    Ok(materialized)
}

fn materialize_forward_body(body: Vec<u8>, credentials: &[ProxyCredential]) -> Result<Vec<u8>> {
    if body.is_empty() {
        return Ok(body);
    }
    match String::from_utf8(body) {
        Ok(text) => Ok(replace_secret_placeholders(&text, credentials)?.into_bytes()),
        Err(err) => Ok(err.into_bytes()),
    }
}

fn replace_secret_placeholders(value: &str, credentials: &[ProxyCredential]) -> Result<String> {
    let mut replaced = value.to_string();
    if let Some(primary) = credentials.first() {
        if let Some(token) = credential_token(primary)? {
            for placeholder in API_KEY_PLACEHOLDERS {
                replaced = replaced.replace(placeholder, &token);
            }
        }
    }

    for credential in credentials {
        for (field, field_value) in credential_fields(credential)? {
            for placeholder in alias_placeholders(&credential.alias, &field) {
                replaced = replaced.replace(&placeholder, &field_value);
            }
        }
    }
    Ok(replaced)
}

fn credential_token(credential: &ProxyCredential) -> Result<Option<String>> {
    Ok(match &credential.secret {
        ProviderSecret::Opaque(_) => None,
        ProviderSecret::ApiKey(secret) => Some(secret.api_key.clone()),
        ProviderSecret::OAuth(secret) => {
            if secret.access_token.is_empty() {
                anyhow::bail!("OAuth account has no access token; run authorization first");
            }
            Some(secret.access_token.clone())
        }
    })
}

fn credential_fields(credential: &ProxyCredential) -> Result<HashMap<String, String>> {
    let mut fields = HashMap::from([
        ("ACCOUNT_ID".into(), credential.account.id.clone()),
        ("ACCOUNT_NAME".into(), credential.account.name.clone()),
        ("PROVIDER".into(), credential.account.provider.to_string()),
        ("BASE_URL".into(), credential.account.base_url.clone()),
    ]);

    match &credential.secret {
        ProviderSecret::Opaque(secret) => {
            for (key, value) in &secret.fields {
                fields.insert(normalize_placeholder_field(key)?, value.clone());
            }
        }
        ProviderSecret::ApiKey(secret) => {
            fields.insert("API_KEY".into(), secret.api_key.clone());
            fields.insert("TOKEN".into(), secret.api_key.clone());
            fields.insert("HEADER_NAME".into(), secret.header_name.clone());
            fields.insert("HEADER_PREFIX".into(), secret.header_prefix.clone());
            if !secret.secret.is_empty() {
                fields.insert("SECRET".into(), secret.secret.clone());
            }
        }
        ProviderSecret::OAuth(secret) => {
            if secret.access_token.is_empty() {
                anyhow::bail!("OAuth account has no access token; run authorization first");
            }
            fields.insert("TOKEN".into(), secret.access_token.clone());
            fields.insert("ACCESS_TOKEN".into(), secret.access_token.clone());
            fields.insert("CLIENT_ID".into(), secret.client_id.clone());
            fields.insert("SCOPES".into(), secret.scopes.join(" "));
        }
    }

    Ok(fields)
}

fn alias_placeholders(alias: &str, field: &str) -> [String; 3] {
    let key = format!("PASSKA_{alias}_{field}");
    [key.clone(), format!("${key}"), format!("{{{{{key}}}}}")]
}

fn normalize_placeholder_alias(alias: &str) -> Result<String> {
    normalize_placeholder_part(alias, "lease alias")
}

fn normalize_placeholder_field(field: &str) -> Result<String> {
    normalize_placeholder_part(field, "secret field")
}

fn normalize_placeholder_part(value: &str, label: &str) -> Result<String> {
    let normalized = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if normalized.is_empty() {
        anyhow::bail!("{label} cannot be empty");
    }
    Ok(normalized)
}

fn is_passka_control_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("x-passka-lease")
        || name.eq_ignore_ascii_case("x-passka-target")
        || name.eq_ignore_ascii_case("x-passka-extra-leases")
        || name.eq_ignore_ascii_case("proxy-authorization")
}

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn build_proxy_url(account: &ProviderAccount, path: &str) -> Result<String> {
    if path.starts_with("http://") || path.starts_with("https://") {
        return Ok(path.to_string());
    }
    if account.base_url.trim().is_empty() {
        anyhow::bail!("account '{}' has no base_url configured", account.name);
    }
    let base = account.base_url.trim_end_matches('/');
    let suffix = path.trim_start_matches('/');
    Ok(format!("{base}/{suffix}"))
}

fn validate_proxy_url(target_url: &str) -> Result<()> {
    let url = url::Url::parse(target_url).context("invalid proxy target URL")?;
    match url.scheme() {
        "http" | "https" => Ok(()),
        scheme => anyhow::bail!("unsupported proxy target scheme '{scheme}'"),
    }
}

fn resource_matches(pattern: &str, resource: &str) -> bool {
    if pattern.ends_with('*') {
        return resource.starts_with(pattern.trim_end_matches('*'));
    }
    pattern == resource
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn secret_key(account_id: &str) -> String {
    format!("provider-account:{account_id}")
}

fn new_id(prefix: &str) -> String {
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{counter}", Utc::now().timestamp_micros())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ApiKeyMaterial, ProviderKind};
    use axum::{extract::State, http::HeaderMap, routing::get, routing::post, Router};
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tempfile::TempDir;

    struct TestEnv {
        _temp: Arc<TempDir>,
        broker: Broker,
    }

    fn with_temp_home() -> TestEnv {
        let temp = Arc::new(tempfile::tempdir().unwrap());
        let broker = Broker::from_dir(temp.path().join("passka").join("broker")).unwrap();
        TestEnv {
            _temp: temp,
            broker,
        }
    }

    #[test]
    fn access_denied_is_audited() {
        let env = with_temp_home();
        let result = env.broker.request_access(
            "principal:local-agent",
            "github/repos/demo",
            "read",
            AccessContext::default(),
        );
        assert!(result.is_err());
        let events = env.broker.list_audit_events(Some(1)).unwrap();
        assert_eq!(events[0].kind, AuditEventKind::AccessDenied);
        drop(env);
    }

    #[test]
    fn human_secret_reveal_is_allowed() {
        let env = with_temp_home();
        let account = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "openai".into(),
                provider: ProviderKind::OpenAI,
                base_url: "https://api.openai.com".into(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "sk-test".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        let value = env
            .broker
            .reveal_sensitive_field("principal:local-human", &account.id, "api_key")
            .unwrap();
        assert_eq!(value, "sk-test");
        drop(env);
    }

    #[test]
    fn proxy_request_uses_lease() {
        let env = with_temp_home();
        let account = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "github".into(),
                provider: ProviderKind::GitHub,
                base_url: String::new(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "abc123".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        env.broker
            .allow_policy(
                "principal:local-agent",
                &account.id,
                "github/repos/*",
                vec!["read".into()],
                vec![],
                DEFAULT_LEASE_SECONDS,
                false,
                "",
            )
            .unwrap();

        async fn handler(headers: HeaderMap, State(token): State<String>) -> String {
            let authorized = headers
                .get("authorization")
                .and_then(|header| header.to_str().ok())
                .unwrap_or_default()
                .replace("Bearer ", "")
                == token;
            let placeholder_replaced = headers
                .get("x-api-key")
                .and_then(|header| header.to_str().ok())
                .unwrap_or_default()
                == token;
            if authorized && placeholder_replaced {
                "ok".to_string()
            } else {
                "missing".to_string()
            }
        }

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let app = Router::new()
            .route("/repos/demo", get(handler))
            .with_state("abc123".to_string());
        let (listener, addr): (tokio::net::TcpListener, SocketAddr) = runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr: SocketAddr = listener.local_addr().unwrap();
            (listener, addr)
        });
        runtime.spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let broker = &env.broker;
        let mut account_meta = broker.get_account(&account.id).unwrap();
        account_meta.base_url = format!("http://{addr}");
        let mut state = broker.load_state().unwrap();
        let stored = state
            .accounts
            .iter_mut()
            .find(|stored| stored.id == account.id)
            .unwrap();
        stored.base_url = account_meta.base_url.clone();
        broker.save_state(&state).unwrap();

        let lease = broker
            .request_access(
                "principal:local-agent",
                "github/repos/demo",
                "read",
                AccessContext {
                    environment: "test".into(),
                    purpose: "unit test".into(),
                    source: String::new(),
                },
            )
            .unwrap();
        let response = broker
            .proxy_http(
                &lease.id,
                HttpRequestSpec {
                    method: "GET".into(),
                    path: "/repos/demo".into(),
                    headers: HashMap::from([(
                        "x-api-key".to_string(),
                        "PASSKA_API_KEY".to_string(),
                    )]),
                    body: String::new(),
                },
            )
            .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "ok");
        drop(env);
    }

    #[test]
    fn forward_proxy_replaces_key_placeholders() {
        let env = with_temp_home();
        let account = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "openai-forward".into(),
                provider: ProviderKind::OpenAI,
                base_url: String::new(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "sk-forward".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        env.broker
            .allow_policy(
                "principal:local-agent",
                &account.id,
                "openai/chat/*",
                vec!["write".into()],
                vec![],
                DEFAULT_LEASE_SECONDS,
                false,
                "",
            )
            .unwrap();

        async fn handler(headers: HeaderMap, body: String) -> String {
            let authorized = headers
                .get("authorization")
                .and_then(|header| header.to_str().ok())
                .unwrap_or_default()
                == "Bearer sk-forward";
            if authorized && body.contains("sk-forward") && !body.contains("PASSKA_API_KEY") {
                "ok".to_string()
            } else {
                "missing".to_string()
            }
        }

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let app = Router::new().route("/v1/chat/completions", post(handler));
        let (listener, addr): (tokio::net::TcpListener, SocketAddr) = runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr: SocketAddr = listener.local_addr().unwrap();
            (listener, addr)
        });
        runtime.spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let lease = env
            .broker
            .request_access(
                "principal:local-agent",
                "openai/chat/completions",
                "write",
                AccessContext {
                    environment: "test".into(),
                    purpose: "unit test".into(),
                    source: String::new(),
                },
            )
            .unwrap();
        let response = env
            .broker
            .proxy_forward_http(
                &lease.id,
                "POST",
                &format!("http://{addr}/v1/chat/completions"),
                HashMap::from([("content-type".to_string(), "application/json".to_string())]),
                br#"{"api_key":"PASSKA_API_KEY"}"#.to_vec(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "ok");
        drop(env);
    }

    #[test]
    fn proxy_request_replaces_extra_lease_placeholders() {
        let env = with_temp_home();
        let primary = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "openai-primary".into(),
                provider: ProviderKind::OpenAI,
                base_url: String::new(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "sk-primary".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        let extra = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "github-extra".into(),
                provider: ProviderKind::GitHub,
                base_url: String::new(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "gh-extra".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();

        env.broker
            .allow_policy(
                "principal:local-agent",
                &primary.id,
                "combo/request",
                vec!["write".into()],
                vec![],
                DEFAULT_LEASE_SECONDS,
                false,
                "",
            )
            .unwrap();
        env.broker
            .allow_policy(
                "principal:local-agent",
                &extra.id,
                "github/tokens/read",
                vec!["read".into()],
                vec![],
                DEFAULT_LEASE_SECONDS,
                false,
                "",
            )
            .unwrap();

        async fn handler(
            headers: HeaderMap,
            State(extra_account_id): State<String>,
            body: String,
        ) -> String {
            let authorized = headers
                .get("authorization")
                .and_then(|header| header.to_str().ok())
                .unwrap_or_default()
                == "Bearer sk-primary";
            let replaced = body.contains("sk-primary")
                && body.contains("gh-extra")
                && body.contains(&extra_account_id)
                && !body.contains("PASSKA_");
            if authorized && replaced {
                "ok".to_string()
            } else {
                "missing".to_string()
            }
        }

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let app = Router::new()
            .route("/combo", post(handler))
            .with_state(extra.id.clone());
        let (listener, addr): (tokio::net::TcpListener, SocketAddr) = runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr: SocketAddr = listener.local_addr().unwrap();
            (listener, addr)
        });
        runtime.spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let primary_lease = env
            .broker
            .request_access(
                "principal:local-agent",
                "combo/request",
                "write",
                AccessContext::default(),
            )
            .unwrap();
        let extra_lease = env
            .broker
            .request_access(
                "principal:local-agent",
                "github/tokens/read",
                "read",
                AccessContext::default(),
            )
            .unwrap();

        let response = env
            .broker
            .proxy_forward_http_with_leases(
                &primary_lease.id,
                HashMap::from([("github".to_string(), extra_lease.id)]),
                "POST",
                &format!("http://{addr}/combo"),
                HashMap::from([("content-type".to_string(), "application/json".to_string())]),
                br#"{"primary":"PASSKA_API_KEY","github":"PASSKA_GITHUB_API_KEY","github_account":"PASSKA_GITHUB_ACCOUNT_ID"}"#.to_vec(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "ok");
        drop(env);
    }
}
