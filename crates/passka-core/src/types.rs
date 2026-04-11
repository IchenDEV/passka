use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalKind {
    Human,
    Agent,
}

impl PrincipalKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
        }
    }
}

impl std::fmt::Display for PrincipalKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PrincipalKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            _ => Err(format!("unknown principal kind '{s}'. valid: human, agent")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    GenericApi,
    OpenAI,
    GitHub,
    Slack,
    Feishu,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GenericApi => "generic_api",
            Self::OpenAI => "openai",
            Self::GitHub => "github",
            Self::Slack => "slack",
            Self::Feishu => "feishu",
        }
    }
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ProviderKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "generic_api" => Ok(Self::GenericApi),
            "openai" => Ok(Self::OpenAI),
            "github" => Ok(Self::GitHub),
            "slack" => Ok(Self::Slack),
            "feishu" => Ok(Self::Feishu),
            _ => Err(format!(
                "unknown provider '{s}'. valid: generic_api, openai, github, slack, feishu"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    Opaque,
    ApiKey,
    OAuth,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::ApiKey => "api_key",
            Self::OAuth => "oauth",
        }
    }
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AuthMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "opaque" => Ok(Self::Opaque),
            "api_key" => Ok(Self::ApiKey),
            "oauth" => Ok(Self::OAuth),
            _ => Err(format!(
                "unknown auth method '{s}'. valid: opaque, api_key, oauth"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Principal {
    pub id: String,
    pub name: String,
    pub kind: PrincipalKind,
    #[serde(default)]
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub id: String,
    pub name: String,
    pub provider: ProviderKind,
    pub auth_method: AuthMethod,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyMaterial {
    pub api_key: String,
    #[serde(default = "default_header_name")]
    pub header_name: String,
    #[serde(default)]
    pub header_prefix: String,
    #[serde(default)]
    pub secret: String,
}

fn default_header_name() -> String {
    "Authorization".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthMaterial {
    pub authorize_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default = "default_redirect_uri")]
    pub redirect_uri: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: String,
}

fn default_redirect_uri() -> String {
    "http://localhost:8477/callback".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpaqueSecretMaterial {
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderSecret {
    Opaque(OpaqueSecretMaterial),
    ApiKey(ApiKeyMaterial),
    OAuth(OAuthMaterial),
}

impl ProviderSecret {
    pub fn auth_method(&self) -> AuthMethod {
        match self {
            Self::Opaque(_) => AuthMethod::Opaque,
            Self::ApiKey(_) => AuthMethod::ApiKey,
            Self::OAuth(_) => AuthMethod::OAuth,
        }
    }

    pub fn reveal_field(&self, field: &str) -> Option<String> {
        match self {
            Self::Opaque(secret) => secret.fields.get(field).cloned(),
            Self::ApiKey(secret) => match field {
                "api_key" => Some(secret.api_key.clone()),
                "header_name" => Some(secret.header_name.clone()),
                "header_prefix" => Some(secret.header_prefix.clone()),
                "secret" if !secret.secret.is_empty() => Some(secret.secret.clone()),
                _ => None,
            },
            Self::OAuth(secret) => match field {
                "authorize_url" => Some(secret.authorize_url.clone()),
                "token_url" => Some(secret.token_url.clone()),
                "client_id" => Some(secret.client_id.clone()),
                "client_secret" => Some(secret.client_secret.clone()),
                "redirect_uri" => Some(secret.redirect_uri.clone()),
                "access_token" if !secret.access_token.is_empty() => Some(secret.access_token.clone()),
                "refresh_token" if !secret.refresh_token.is_empty() => {
                    Some(secret.refresh_token.clone())
                }
                "expires_at" if !secret.expires_at.is_empty() => Some(secret.expires_at.clone()),
                "scopes" => Some(secret.scopes.join(" ")),
                _ => None,
            },
        }
    }

    pub fn fields_for_display(&self) -> Vec<String> {
        match self {
            Self::Opaque(secret) => {
                let mut keys: Vec<_> = secret.fields.keys().cloned().collect();
                keys.sort();
                keys
            }
            Self::ApiKey(_) => vec![
                "api_key".into(),
                "header_name".into(),
                "header_prefix".into(),
                "secret".into(),
            ],
            Self::OAuth(_) => vec![
                "authorize_url".into(),
                "token_url".into(),
                "client_id".into(),
                "client_secret".into(),
                "redirect_uri".into(),
                "scopes".into(),
                "access_token".into(),
                "refresh_token".into(),
                "expires_at".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceGrant {
    pub id: String,
    pub account_id: String,
    pub resource: String,
    pub actions: Vec<String>,
    #[serde(default)]
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerPolicy {
    pub id: String,
    pub principal_id: String,
    pub grant_ids: Vec<String>,
    #[serde(default)]
    pub environments: Vec<String>,
    pub allow_secret_reveal: bool,
    pub max_lease_seconds: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccessContext {
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLease {
    pub id: String,
    pub principal_id: String,
    pub account_id: String,
    pub grant_id: String,
    pub resource: String,
    pub action: String,
    pub expires_at: String,
    pub created_at: String,
    pub context: AccessContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    PrincipalCreated,
    AccountRegistered,
    AuthorizationStarted,
    AuthorizationCompleted,
    TokenRefreshed,
    PolicyCreated,
    AccessGranted,
    AccessDenied,
    ProxyRequest,
    SecretViewed,
    SecretRevealDenied,
    AccountRemoved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Denied,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: String,
    #[serde(default)]
    pub actor_principal_id: String,
    pub kind: AuditEventKind,
    pub outcome: AuditOutcome,
    #[serde(default)]
    pub resource: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationSession {
    pub account_id: String,
    pub authorization_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequestSpec {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProxyResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterProviderAccount {
    pub name: String,
    pub provider: ProviderKind,
    pub base_url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub secret: ProviderSecret,
}

pub fn mask_value(val: &str) -> String {
    let len = val.len();
    if len <= 4 {
        return "*".repeat(len.max(1));
    }
    let visible = &val[len - 4..];
    format!("{}****{}", &val[..2.min(len)], visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_value() {
        assert_eq!(mask_value("sk-abc123def456"), "sk****f456");
        assert_eq!(mask_value("ab"), "**");
    }

    #[test]
    fn test_provider_secret_fields() {
        let secret = ProviderSecret::ApiKey(ApiKeyMaterial {
            api_key: "secret".into(),
            header_name: "Authorization".into(),
            header_prefix: "Bearer".into(),
            secret: String::new(),
        });
        assert_eq!(secret.auth_method(), AuthMethod::ApiKey);
        assert_eq!(secret.reveal_field("api_key").as_deref(), Some("secret"));
    }
}
