use crate::oauth;
use crate::store::keychain::KeychainStore;
use crate::types::{
    AccessContext, AccessLease, AccountAuthorization, AuditEvent, AuditEventKind, AuditOutcome,
    AuthorizationSession, HttpProxyResponse, HttpRequestSpec, Principal, PrincipalKind,
    ProviderAccount, ProviderSecret, RegisterProviderAccount,
};
use anyhow::{Context, Result};
use chrono::Utc;
use data_encoding::HEXLOWER;
use rand::Rng;
use reqwest::header::{HeaderMap as ReqwestHeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const BROKER_SERVICE_NAME: &str = "passka-broker";
#[cfg(test)]
const DEFAULT_LEASE_SECONDS: i64 = 300;
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct ProxyCredential {
    lease: AccessLease,
    secret: ProviderSecret,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentTokenRecord {
    principal_id: String,
    token_hash: String,
    created_at: String,
    #[serde(default)]
    rotated_at: String,
    #[serde(default)]
    revoked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct BrokerState {
    principals: Vec<Principal>,
    agent_tokens: Vec<AgentTokenRecord>,
    accounts: Vec<ProviderAccount>,
    authorizations: Vec<AccountAuthorization>,
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

    pub fn from_dir(config_dir: PathBuf) -> Result<Self> {
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

    pub fn issue_agent_token(&self, principal_id: &str) -> Result<String> {
        let mut state = self.load_state()?;
        let principal = state
            .principals
            .iter()
            .find(|principal| principal.id == principal_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("principal '{principal_id}' not found"))?;
        if principal.kind != PrincipalKind::Agent {
            anyhow::bail!("principal '{principal_id}' is not an agent");
        }

        let issued_at = now();
        for token in state
            .agent_tokens
            .iter_mut()
            .filter(|token| token.principal_id == principal_id && token.rotated_at.is_empty() && token.revoked_at.is_empty())
        {
            token.rotated_at = issued_at.clone();
        }

        let token = generate_agent_token();
        state.agent_tokens.push(AgentTokenRecord {
            principal_id: principal_id.to_string(),
            token_hash: hash_agent_token(&token),
            created_at: issued_at.clone(),
            rotated_at: String::new(),
            revoked_at: String::new(),
        });
        self.append_audit(
            &mut state,
            principal_id,
            AuditEventKind::AgentTokenIssued,
            AuditOutcome::Success,
            principal_id.to_string(),
            "issued agent token".into(),
        );
        self.save_state(&state)?;
        Ok(token)
    }

    pub fn revoke_agent_token(&self, principal_id: &str) -> Result<()> {
        let mut state = self.load_state()?;
        let principal = state
            .principals
            .iter()
            .find(|principal| principal.id == principal_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("principal '{principal_id}' not found"))?;
        if principal.kind != PrincipalKind::Agent {
            anyhow::bail!("principal '{principal_id}' is not an agent");
        }

        let revoked_at = now();
        let Some(token) = state.agent_tokens.iter_mut().rev().find(|token| {
            token.principal_id == principal_id
                && token.rotated_at.is_empty()
                && token.revoked_at.is_empty()
        }) else {
            anyhow::bail!("principal '{principal_id}' has no active agent token");
        };
        token.revoked_at = revoked_at;
        self.append_audit(
            &mut state,
            principal_id,
            AuditEventKind::AgentTokenRevoked,
            AuditOutcome::Success,
            principal_id.to_string(),
            "revoked agent token".into(),
        );
        self.save_state(&state)?;
        Ok(())
    }

    pub fn authenticate_agent_token(&self, raw_token: &str) -> Result<Principal> {
        let state = self.load_state()?;
        let token_hash = hash_agent_token(raw_token);
        let record = state
            .agent_tokens
            .iter()
            .rev()
            .find(|token| {
                token.token_hash == token_hash
                    && token.rotated_at.is_empty()
                    && token.revoked_at.is_empty()
            })
            .ok_or_else(|| anyhow::anyhow!("invalid agent token"))?;
        let principal = state
            .principals
            .iter()
            .find(|principal| principal.id == record.principal_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("principal '{}' not found", record.principal_id))?;
        if principal.kind != PrincipalKind::Agent {
            anyhow::bail!("principal '{}' is not an agent", principal.id);
        }
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
        request.secret.validate()?;

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
        state
            .authorizations
            .retain(|authorization| authorization.account_id != account_id);
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

    pub fn authorize_account(
        &self,
        principal_id: &str,
        account_id: &str,
        environments: Vec<String>,
        allowed_hosts: Vec<String>,
        allowed_methods: Vec<String>,
        allowed_path_prefixes: Vec<String>,
        max_lease_seconds: i64,
        description: &str,
    ) -> Result<AccountAuthorization> {
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
        let allowed_hosts = normalize_allowed_hosts(allowed_hosts);
        let allowed_methods = normalize_allowed_methods(allowed_methods);
        let allowed_path_prefixes = normalize_allowed_path_prefixes(allowed_path_prefixes);

        if let Some(existing) = state.authorizations.iter_mut().find(|authorization| {
            authorization.account_id == account_id && authorization.principal_id == principal_id
        }) {
            existing.environments = environments;
            existing.allowed_hosts = allowed_hosts;
            existing.allowed_methods = allowed_methods;
            existing.allowed_path_prefixes = allowed_path_prefixes;
            existing.max_lease_seconds = max_lease_seconds.max(30);
            existing.description = description.to_string();
            existing.updated_at = now();
            let authorization = existing.clone();
            self.append_audit(
                &mut state,
                principal_id,
                AuditEventKind::AccountAuthorized,
                AuditOutcome::Success,
                format!("account:{account_id}"),
                format!("updated account authorization '{}'", authorization.id),
            );
            self.save_state(&state)?;
            return Ok(authorization);
        }

        let now = now();
        let authorization = AccountAuthorization {
            id: new_id("authz"),
            principal_id: principal_id.to_string(),
            account_id: account_id.to_string(),
            environments,
            allowed_hosts,
            allowed_methods,
            allowed_path_prefixes,
            max_lease_seconds: max_lease_seconds.max(30),
            description: description.to_string(),
            created_at: now.clone(),
            updated_at: now,
        };

        state.authorizations.push(authorization.clone());
        self.append_audit(
            &mut state,
            principal_id,
            AuditEventKind::AccountAuthorized,
            AuditOutcome::Success,
            format!("account:{account_id}"),
            format!("authorized account '{}' for agent access", account_id),
        );
        self.save_state(&state)?;
        Ok(authorization)
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

    pub fn request_access(
        &self,
        principal_id: &str,
        account_id: &str,
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
            .authorizations
            .iter()
            .filter(|authorization| authorization.principal_id == principal_id)
            .find_map(|authorization| {
                if authorization.account_id != account_id {
                    return None;
                }
                if !authorization.environments.is_empty() {
                    if context.environment.is_empty() {
                        return None;
                    }
                    if !authorization
                        .environments
                        .iter()
                        .any(|env| env == &context.environment)
                    {
                        return None;
                    }
                }
                Some(authorization.clone())
            });

        let Some(authorization) = decision else {
            self.append_audit(
                &mut state,
                principal_id,
                AuditEventKind::AccessDenied,
                AuditOutcome::Denied,
                format!("account:{account_id}"),
                format!("agent is not authorized for account in '{}'", context.environment),
            );
            self.save_state(&state)?;
            anyhow::bail!("principal '{principal_id}' is not authorized to use account '{account_id}'");
        };
        let account = state
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("account '{account_id}' not found"))?;
        let (allowed_hosts, allowed_methods, allowed_path_prefixes) =
            resolve_lease_scope(&authorization, &account)?;

        let now = now();
        let expires_at = (Utc::now()
            + chrono::Duration::seconds(authorization.max_lease_seconds.max(30)))
        .to_rfc3339();
        let lease = AccessLease {
            id: new_id("lease"),
            principal_id: principal_id.to_string(),
            account_id: account_id.to_string(),
            allowed_hosts,
            allowed_methods,
            allowed_path_prefixes,
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
            format!("account:{account_id}"),
            format!("lease '{}' created", lease.id),
        );
        self.save_state(&state)?;
        Ok(lease)
    }

    pub fn proxy_http(
        &self,
        actor_principal_id: &str,
        lease_id: &str,
        spec: HttpRequestSpec,
    ) -> Result<HttpProxyResponse> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.proxy_http_async(actor_principal_id, lease_id, spec))
    }

    pub async fn proxy_http_async(
        &self,
        actor_principal_id: &str,
        lease_id: &str,
        spec: HttpRequestSpec,
    ) -> Result<HttpProxyResponse> {
        let (lease, account) = self.active_lease_account_for_principal(actor_principal_id, lease_id)?;
        let url = build_proxy_url(&account, &spec.path)?;
        let credential = self.proxy_credential(&lease).await?;
        self.proxy_request_async(
            &credential,
            &spec.method,
            &url,
            spec.headers,
            spec.body.into_bytes(),
        )
        .await
    }

    pub fn proxy_forward_http(
        &self,
        actor_principal_id: &str,
        lease_id: &str,
        method: &str,
        target_url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<HttpProxyResponse> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.proxy_forward_http_async(
            actor_principal_id,
            lease_id,
            method,
            target_url,
            headers,
            body,
        ))
    }

    pub async fn proxy_forward_http_async(
        &self,
        actor_principal_id: &str,
        lease_id: &str,
        method: &str,
        target_url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<HttpProxyResponse> {
        let (lease, _) = self.active_lease_account_for_principal(actor_principal_id, lease_id)?;
        validate_proxy_url(target_url)?;
        let credential = self.proxy_credential(&lease).await?;
        self.proxy_request_async(&credential, method, target_url, headers, body)
            .await
    }

    fn active_lease_account_for_principal(
        &self,
        actor_principal_id: &str,
        lease_id: &str,
    ) -> Result<(AccessLease, ProviderAccount)> {
        let (lease, account) = self.active_lease_account(lease_id)?;
        if lease.principal_id != actor_principal_id {
            anyhow::bail!(
                "lease '{}' belongs to principal '{}', not '{}'",
                lease.id,
                lease.principal_id,
                actor_principal_id
            );
        }
        Ok((lease, account))
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
        credential: &ProxyCredential,
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<HttpProxyResponse> {
        let method_label = method.to_string();
        let method: reqwest::Method = method.parse().context("invalid HTTP method")?;
        validate_proxy_target_scope(&credential.lease, method.as_str(), url)?;
        let headers = materialize_forward_headers(headers, credential)?;
        let body = materialize_forward_body(body);
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
            &credential.lease.principal_id,
            AuditEventKind::ProxyRequest,
            AuditOutcome::Success,
            format!("account:{}", credential.lease.account_id),
            format!("proxied {} {}", method_label, url),
        );
        self.save_state(&state)?;

        Ok(HttpProxyResponse {
            status,
            headers,
            body,
        })
    }

    async fn proxy_credential(&self, lease: &AccessLease) -> Result<ProxyCredential> {
        Ok(ProxyCredential {
            lease: lease.clone(),
            secret: self.load_proxy_secret(lease).await?,
        })
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
        let serialized = serde_json::to_string_pretty(state)?;
        let temp_path = self
            .state_path
            .with_extension(format!("tmp-{}", new_id("state")));
        fs::write(&temp_path, serialized)?;
        match fs::rename(&temp_path, &self.state_path) {
            Ok(()) => Ok(()),
            Err(err) => {
                let _ = fs::remove_file(&temp_path);
                Err(err).context("failed to atomically replace broker state")?
            }
        }
    }

    fn save_secret(&self, account_id: &str, secret: &ProviderSecret) -> Result<()> {
        KeychainStore::set_json(BROKER_SERVICE_NAME, &secret_key(account_id), secret)
    }

    fn load_secret(&self, account_id: &str) -> Result<ProviderSecret> {
        KeychainStore::get_json(BROKER_SERVICE_NAME, &secret_key(account_id))
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
    credential: &ProxyCredential,
) -> Result<ReqwestHeaderMap> {
    let mut materialized = ReqwestHeaderMap::new();
    for (name, value) in headers {
        if is_hop_by_hop_header(&name) || is_passka_control_header(&name) {
            continue;
        }
        let header_name: HeaderName = name
            .parse()
            .with_context(|| format!("invalid request header '{name}'"))?;
        let header_value: HeaderValue = value
            .parse()
            .with_context(|| format!("invalid request header value for '{name}'"))?;
        materialized.insert(header_name, header_value);
    }

    match &credential.secret {
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
        ProviderSecret::Otp(_) => anyhow::bail!("OTP secrets cannot be proxied over HTTP"),
    }

    Ok(materialized)
}

fn materialize_forward_body(body: Vec<u8>) -> Vec<u8> {
    body
}

fn is_passka_control_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("x-passka-lease")
        || name.eq_ignore_ascii_case("x-passka-agent-token")
        || name.eq_ignore_ascii_case("x-passka-target")
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

fn normalize_allowed_hosts(hosts: Vec<String>) -> Vec<String> {
    dedup_strings(
        hosts
            .into_iter()
            .map(|host| host.trim().trim_matches('/').to_ascii_lowercase())
            .filter(|host| !host.is_empty())
            .collect(),
    )
}

fn normalize_allowed_methods(methods: Vec<String>) -> Vec<String> {
    dedup_strings(
        methods
            .into_iter()
            .map(|method| method.trim().to_ascii_uppercase())
            .filter(|method| !method.is_empty())
            .collect(),
    )
}

fn normalize_allowed_path_prefixes(prefixes: Vec<String>) -> Vec<String> {
    dedup_strings(
        prefixes
            .into_iter()
            .map(|prefix| {
                let trimmed = prefix.trim();
                if trimmed.is_empty() {
                    String::new()
                } else if trimmed.starts_with('/') {
                    trim_path_prefix(trimmed)
                } else {
                    trim_path_prefix(&format!("/{trimmed}"))
                }
            })
            .filter(|prefix| !prefix.is_empty())
            .collect(),
    )
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.iter().any(|existing| existing == &value) {
            unique.push(value);
        }
    }
    unique
}

fn trim_path_prefix(prefix: &str) -> String {
    if prefix == "/" {
        "/".into()
    } else {
        prefix.trim_end_matches('/').to_string()
    }
}

fn resolve_lease_scope(
    authorization: &AccountAuthorization,
    account: &ProviderAccount,
) -> Result<(Vec<String>, Vec<String>, Vec<String>)> {
    let mut allowed_hosts = authorization.allowed_hosts.clone();
    if allowed_hosts.is_empty() {
        allowed_hosts = derived_hosts_from_base_url(&account.base_url)?;
    }

    let mut allowed_path_prefixes = authorization.allowed_path_prefixes.clone();
    if allowed_path_prefixes.is_empty() {
        allowed_path_prefixes = derived_path_prefixes_from_base_url(&account.base_url)?;
    }

    Ok((
        allowed_hosts,
        authorization.allowed_methods.clone(),
        allowed_path_prefixes,
    ))
}

fn derived_hosts_from_base_url(base_url: &str) -> Result<Vec<String>> {
    if base_url.trim().is_empty() {
        return Ok(Vec::new());
    }
    let url = url::Url::parse(base_url).context("invalid account base_url")?;
    let mut hosts = Vec::new();
    if let Some(host) = url.host_str() {
        hosts.push(host.to_ascii_lowercase());
    }
    if let Some(authority) = url.host_str().map(|host| {
        url.port()
            .map(|port| format!("{}:{port}", host.to_ascii_lowercase()))
            .unwrap_or_else(|| host.to_ascii_lowercase())
    }) {
        if !hosts.iter().any(|existing| existing == &authority) {
            hosts.push(authority);
        }
    }
    Ok(hosts)
}

fn derived_path_prefixes_from_base_url(base_url: &str) -> Result<Vec<String>> {
    if base_url.trim().is_empty() {
        return Ok(Vec::new());
    }
    let url = url::Url::parse(base_url).context("invalid account base_url")?;
    let path = trim_path_prefix(url.path());
    if path == "/" {
        return Ok(Vec::new());
    }
    Ok(vec![path])
}

fn validate_proxy_target_scope(lease: &AccessLease, method: &str, target_url: &str) -> Result<()> {
    let url = url::Url::parse(target_url).context("invalid proxy target URL")?;
    let host = url
        .host_str()
        .map(|host| host.to_ascii_lowercase())
        .ok_or_else(|| anyhow::anyhow!("proxy target URL is missing a host"))?;
    let authority = url
        .port()
        .map(|port| format!("{host}:{port}"))
        .unwrap_or_else(|| host.clone());
    let path = trim_path_prefix(url.path());
    let method = method.to_ascii_uppercase();

    if !lease.allowed_hosts.is_empty()
        && !lease
            .allowed_hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(&host) || allowed.eq_ignore_ascii_case(&authority))
    {
        anyhow::bail!(
            "lease '{}' is not allowed to access host '{}'",
            lease.id,
            authority
        );
    }
    if lease.allowed_hosts.is_empty() {
        anyhow::bail!(
            "lease '{}' has no allowed host scope; configure account allow --allow-host or set account base_url",
            lease.id
        );
    }

    if !lease.allowed_methods.is_empty()
        && !lease
            .allowed_methods
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(&method))
    {
        anyhow::bail!(
            "lease '{}' is not allowed to use method '{}'",
            lease.id,
            method
        );
    }

    if !lease.allowed_path_prefixes.is_empty()
        && !lease.allowed_path_prefixes.iter().any(|prefix| {
            prefix == "/" || path == *prefix || path.starts_with(&format!("{prefix}/"))
        })
    {
        anyhow::bail!(
            "lease '{}' is not allowed to access path '{}'",
            lease.id,
            path
        );
    }

    Ok(())
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

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn generate_agent_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill(&mut bytes);
    format!("ptok_{}", HEXLOWER.encode(&bytes))
}

fn hash_agent_token(raw_token: &str) -> String {
    HEXLOWER.encode(&Sha256::digest(raw_token.as_bytes()))
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
            "account-missing",
            AccessContext::default(),
        );
        assert!(result.is_err());
        let events = env.broker.list_audit_events(Some(1)).unwrap();
        assert_eq!(events[0].kind, AuditEventKind::AccessDenied);
        drop(env);
    }

    #[test]
    fn issue_and_revoke_agent_token_controls_authentication() {
        let env = with_temp_home();
        let token = env
            .broker
            .issue_agent_token("principal:local-agent")
            .unwrap();
        let principal = env.broker.authenticate_agent_token(&token).unwrap();
        assert_eq!(principal.id, "principal:local-agent");

        let rotated = env
            .broker
            .issue_agent_token("principal:local-agent")
            .unwrap();
        assert!(env.broker.authenticate_agent_token(&token).is_err());
        assert_eq!(
            env.broker.authenticate_agent_token(&rotated).unwrap().id,
            "principal:local-agent"
        );

        env.broker
            .revoke_agent_token("principal:local-agent")
            .unwrap();
        assert!(env.broker.authenticate_agent_token(&rotated).is_err());
    }

    #[test]
    fn environment_must_be_present_when_authorization_restricts_it() {
        let env = with_temp_home();
        let account = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "openai-env".into(),
                provider: ProviderKind::OpenAI,
                base_url: "https://api.openai.com".into(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "sk-env".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        env.broker
            .authorize_account(
                "principal:local-agent",
                &account.id,
                vec!["prod".into()],
                vec![],
                vec![],
                vec![],
                DEFAULT_LEASE_SECONDS,
                "",
            )
            .unwrap();

        let result = env
            .broker
            .request_access("principal:local-agent", &account.id, AccessContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn state_save_remains_valid_json_after_multiple_updates() {
        let env = with_temp_home();
        for idx in 0..5 {
            env.broker
                .add_principal(
                    &format!("agent-{idx}"),
                    PrincipalKind::Agent,
                    "test principal",
                )
                .unwrap();
            let content = fs::read_to_string(&env.broker.state_path).unwrap();
            let state: BrokerState = serde_json::from_str(&content).unwrap();
            assert!(state.principals.len() >= 2);
        }
    }

    #[test]
    fn authorize_account_updates_existing_authorization() {
        let env = with_temp_home();
        let account = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "openai-update".into(),
                provider: ProviderKind::OpenAI,
                base_url: "https://api.openai.com".into(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "sk-update".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();

        let first = env
            .broker
            .authorize_account(
                "principal:local-agent",
                &account.id,
                vec!["local".into()],
                vec!["api.openai.com".into()],
                vec!["GET".into()],
                vec!["/v1/models".into()],
                120,
                "first pass",
            )
            .unwrap();
        let second = env
            .broker
            .authorize_account(
                "principal:local-agent",
                &account.id,
                vec!["prod".into()],
                vec!["api.openai.com".into(), "localhost".into()],
                vec!["POST".into()],
                vec!["/v1/chat".into()],
                600,
                "second pass",
            )
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.environments, vec!["prod".to_string()]);
        assert_eq!(
            second.allowed_hosts,
            vec!["api.openai.com".to_string(), "localhost".to_string()]
        );
        assert_eq!(second.allowed_methods, vec!["POST".to_string()]);
        assert_eq!(second.allowed_path_prefixes, vec!["/v1/chat".to_string()]);
        assert_eq!(second.max_lease_seconds, 600);
        assert_eq!(second.description, "second pass");
        assert_eq!(env.broker.load_state().unwrap().authorizations.len(), 1);
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
            .authorize_account(
                "principal:local-agent",
                &account.id,
                vec![],
                vec![],
                vec![],
                vec![],
                DEFAULT_LEASE_SECONDS,
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
            let forwarded = headers
                .get("x-client")
                .and_then(|header| header.to_str().ok())
                .unwrap_or_default()
                == "passka-test";
            if authorized && forwarded {
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
                &account.id,
                AccessContext {
                    environment: "test".into(),
                    purpose: "unit test".into(),
                    source: String::new(),
                },
            )
            .unwrap();
        let response = broker
            .proxy_http(
                "principal:local-agent",
                &lease.id,
                HttpRequestSpec {
                    method: "GET".into(),
                    path: "/repos/demo".into(),
                    headers: HashMap::from([("x-client".to_string(), "passka-test".to_string())]),
                    body: String::new(),
                },
            )
            .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "ok");
        drop(env);
    }

    #[test]
    fn forward_proxy_injects_auth_header() {
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
            .authorize_account(
                "principal:local-agent",
                &account.id,
                vec![],
                vec!["127.0.0.1".into()],
                vec![],
                vec![],
                DEFAULT_LEASE_SECONDS,
                "",
            )
            .unwrap();

        async fn handler(headers: HeaderMap, body: String) -> String {
            let authorized = headers
                .get("authorization")
                .and_then(|header| header.to_str().ok())
                .unwrap_or_default()
                == "Bearer sk-forward";
            if authorized && body == r#"{"message":"ok"}"# {
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
                &account.id,
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
                "principal:local-agent",
                &lease.id,
                "POST",
                &format!("http://{addr}/v1/chat/completions"),
                HashMap::from([("content-type".to_string(), "application/json".to_string())]),
                br#"{"message":"ok"}"#.to_vec(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "ok");
        drop(env);
    }

    #[test]
    fn lease_derives_default_host_scope_from_base_url() {
        let env = with_temp_home();
        let account = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "openai-scoped".into(),
                provider: ProviderKind::OpenAI,
                base_url: "https://api.openai.com/v1".into(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "sk-scope".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        env.broker
            .authorize_account(
                "principal:local-agent",
                &account.id,
                vec![],
                vec![],
                vec![],
                vec![],
                DEFAULT_LEASE_SECONDS,
                "",
            )
            .unwrap();

        let lease = env
            .broker
            .request_access("principal:local-agent", &account.id, AccessContext::default())
            .unwrap();
        assert_eq!(lease.allowed_hosts, vec!["api.openai.com".to_string()]);
        assert_eq!(lease.allowed_path_prefixes, vec!["/v1".to_string()]);
    }

    #[test]
    fn proxy_scope_rejects_disallowed_method_and_path() {
        let env = with_temp_home();
        let account = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "scoped-account".into(),
                provider: ProviderKind::GenericApi,
                base_url: String::new(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "scoped-key".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        env.broker
            .authorize_account(
                "principal:local-agent",
                &account.id,
                vec![],
                vec!["api.example.com".into()],
                vec!["GET".into()],
                vec!["/v1/models".into()],
                DEFAULT_LEASE_SECONDS,
                "",
            )
            .unwrap();
        let lease = env
            .broker
            .request_access("principal:local-agent", &account.id, AccessContext::default())
            .unwrap();

        let method_denied = env.broker.proxy_forward_http(
            "principal:local-agent",
            &lease.id,
            "POST",
            "https://api.example.com/v1/models",
            HashMap::new(),
            Vec::new(),
        );
        assert!(method_denied
            .unwrap_err()
            .to_string()
            .contains("not allowed to use method"));

        let path_denied = env.broker.proxy_forward_http(
            "principal:local-agent",
            &lease.id,
            "GET",
            "https://api.example.com/v1/chat/completions",
            HashMap::new(),
            Vec::new(),
        );
        assert!(path_denied
            .unwrap_err()
            .to_string()
            .contains("not allowed to access path"));
    }

    #[test]
    fn helper_functions_cover_proxy_path_edge_cases() {
        let account = ProviderAccount {
            id: "account-1".into(),
            name: "demo".into(),
            provider: ProviderKind::OpenAI,
            auth_method: crate::types::AuthMethod::ApiKey,
            base_url: "https://api.openai.com/".into(),
            description: String::new(),
            scopes: vec![],
            created_at: now(),
            updated_at: now(),
        };

        assert_eq!(
            build_proxy_url(&account, "/v1/models").unwrap(),
            "https://api.openai.com/v1/models"
        );
        assert_eq!(
            build_proxy_url(&account, "https://example.com/raw").unwrap(),
            "https://example.com/raw"
        );
        assert!(validate_proxy_url("https://example.com/path").is_ok());
        assert!(validate_proxy_url("ftp://example.com/path").is_err());
        assert!(is_passka_control_header("X-Passka-Lease"));
        assert!(is_hop_by_hop_header("Transfer-Encoding"));
    }

    #[test]
    fn materialize_forward_helpers_cover_auth_and_body_paths() {
        let credential = ProxyCredential {
            lease: AccessLease {
                id: "lease-1".into(),
                principal_id: "principal:local-agent".into(),
                account_id: "account-1".into(),
                allowed_hosts: vec!["api.openai.com".into()],
                allowed_methods: vec![],
                allowed_path_prefixes: vec![],
                expires_at: now(),
                created_at: now(),
                context: AccessContext::default(),
            },
            secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                api_key: "sk-helper".into(),
                header_name: "Authorization".into(),
                header_prefix: "Bearer".into(),
                secret: "secondary".into(),
            }),
        };

        let headers = materialize_forward_headers(
            HashMap::from([
                ("x-passka-lease".into(), "ignored".into()),
                ("x-custom".into(), "plain-value".into()),
            ]),
            &credential,
        )
        .unwrap();
        assert_eq!(
            headers.get("authorization").unwrap().to_str().unwrap(),
            "Bearer sk-helper"
        );
        assert_eq!(headers.get("x-custom").unwrap().to_str().unwrap(), "plain-value");

        let body = materialize_forward_body(br#"{"token":"unchanged"}"#.to_vec());
        assert_eq!(String::from_utf8(body).unwrap(), r#"{"token":"unchanged"}"#);

        let non_utf8 = materialize_forward_body(vec![0xff, 0xfe]);
        assert_eq!(non_utf8, vec![0xff, 0xfe]);
    }

    #[test]
    fn materialize_forward_headers_supports_oauth_auth_injection() {
        let credential = ProxyCredential {
            lease: AccessLease {
                id: "lease-oauth".into(),
                principal_id: "principal:local-agent".into(),
                account_id: "account-oauth".into(),
                allowed_hosts: vec!["slack.com".into()],
                allowed_methods: vec![],
                allowed_path_prefixes: vec![],
                expires_at: now(),
                created_at: now(),
                context: AccessContext::default(),
            },
            secret: ProviderSecret::OAuth(crate::types::OAuthMaterial {
                authorize_url: "https://slack.com/oauth".into(),
                token_url: "https://slack.com/api/oauth.v2.access".into(),
                client_id: "client-id".into(),
                client_secret: "client-secret".into(),
                redirect_uri: "http://localhost:8477/callback".into(),
                scopes: vec!["chat:write".into()],
                access_token: "xoxb-token".into(),
                refresh_token: String::new(),
                expires_at: String::new(),
            }),
        };

        let headers = materialize_forward_headers(HashMap::new(), &credential).unwrap();
        assert_eq!(
            headers.get("authorization").unwrap().to_str().unwrap(),
            "Bearer xoxb-token"
        );
    }

    #[test]
    fn account_authorizations_are_account_scoped() {
        let authorization = AccountAuthorization {
            id: "authz-1".into(),
            principal_id: "principal:local-agent".into(),
            account_id: "account-openai".into(),
            environments: vec!["local".into()],
            allowed_hosts: vec!["api.openai.com".into()],
            allowed_methods: vec!["GET".into()],
            allowed_path_prefixes: vec!["/v1".into()],
            max_lease_seconds: 300,
            description: "openai access".into(),
            created_at: now(),
            updated_at: now(),
        };

        assert_eq!(authorization.account_id, "account-openai");
        assert_eq!(authorization.environments, vec!["local".to_string()]);
        assert_eq!(authorization.allowed_hosts, vec!["api.openai.com".to_string()]);
    }
}
