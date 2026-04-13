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
    Otp,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::ApiKey => "api_key",
            Self::OAuth => "oauth",
            Self::Otp => "otp",
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
            "otp" => Ok(Self::Otp),
            _ => Err(format!(
                "unknown auth method '{s}'. valid: opaque, api_key, oauth, otp"
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
pub struct OtpMaterial {
    pub seed: String,
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub account_name: String,
    #[serde(default = "default_otp_digits")]
    pub digits: u32,
    #[serde(default = "default_otp_period")]
    pub period: u64,
}

fn default_otp_digits() -> u32 {
    6
}

fn default_otp_period() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderSecret {
    Opaque(OpaqueSecretMaterial),
    ApiKey(ApiKeyMaterial),
    OAuth(OAuthMaterial),
    Otp(OtpMaterial),
}

impl ProviderSecret {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Self::Otp(secret) = self {
            let _ = totp_at(&secret.seed, 0, secret.digits, secret.period)?;
        }
        Ok(())
    }

    pub fn auth_method(&self) -> AuthMethod {
        match self {
            Self::Opaque(_) => AuthMethod::Opaque,
            Self::ApiKey(_) => AuthMethod::ApiKey,
            Self::OAuth(_) => AuthMethod::OAuth,
            Self::Otp(_) => AuthMethod::Otp,
        }
    }
}

fn totp_at(seed: &str, epoch_seconds: u64, digits: u32, period: u64) -> anyhow::Result<String> {
    use hmac::{Hmac, Mac};
    use sha1::Sha1;

    if !(6..=10).contains(&digits) {
        anyhow::bail!("OTP digits must be between 6 and 10");
    }
    if period == 0 {
        anyhow::bail!("OTP period must be greater than zero");
    }

    let key = decode_base32_seed(seed)?;
    let counter = epoch_seconds / period;
    let mut mac = Hmac::<Sha1>::new_from_slice(&key)?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    let modulo = 10_u64.pow(digits);
    Ok(format!(
        "{:0width$}",
        u64::from(binary) % modulo,
        width = digits as usize
    ))
}

fn decode_base32_seed(seed: &str) -> anyhow::Result<Vec<u8>> {
    let compact: String = seed
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-' && *ch != '=')
        .flat_map(char::to_uppercase)
        .collect();
    if compact.is_empty() {
        anyhow::bail!("OTP seed cannot be empty");
    }
    let padding = (8 - compact.len() % 8) % 8;
    let padded = format!("{compact}{}", "=".repeat(padding));
    data_encoding::BASE32
        .decode(padded.as_bytes())
        .map_err(|err| anyhow::anyhow!("invalid base32 OTP seed: {err}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountAuthorization {
    pub id: String,
    pub principal_id: String,
    pub account_id: String,
    #[serde(default)]
    pub environments: Vec<String>,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub allowed_methods: Vec<String>,
    #[serde(default)]
    pub allowed_path_prefixes: Vec<String>,
    pub max_lease_seconds: i64,
    #[serde(default)]
    pub description: String,
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
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub allowed_methods: Vec<String>,
    #[serde(default)]
    pub allowed_path_prefixes: Vec<String>,
    pub expires_at: String,
    pub created_at: String,
    pub context: AccessContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    PrincipalCreated,
    AgentTokenIssued,
    AgentTokenRevoked,
    AccountRegistered,
    PolicyCreated,
    AccountAuthorized,
    AuthorizationStarted,
    AuthorizationCompleted,
    TokenRefreshed,
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
    fn principal_kind_round_trips_display_and_parse() {
        assert_eq!(PrincipalKind::Human.to_string(), "human");
        assert_eq!("agent".parse::<PrincipalKind>().unwrap(), PrincipalKind::Agent);
        assert!("robot".parse::<PrincipalKind>().is_err());
    }

    #[test]
    fn provider_kind_round_trips_display_and_parse() {
        assert_eq!(ProviderKind::OpenAI.to_string(), "openai");
        assert_eq!(
            "generic_api".parse::<ProviderKind>().unwrap(),
            ProviderKind::GenericApi
        );
        assert!("unknown".parse::<ProviderKind>().is_err());
    }

    #[test]
    fn auth_method_round_trips_display_and_parse() {
        assert_eq!(AuthMethod::OAuth.to_string(), "oauth");
        assert_eq!("otp".parse::<AuthMethod>().unwrap(), AuthMethod::Otp);
        assert!("saml".parse::<AuthMethod>().is_err());
    }

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
    }

    #[test]
    fn opaque_secret_validation_is_supported() {
        let secret = ProviderSecret::Opaque(OpaqueSecretMaterial {
            fields: HashMap::from([
                ("z_last".into(), "2".into()),
                ("a_first".into(), "1".into()),
            ]),
        });
        assert_eq!(secret.auth_method(), AuthMethod::Opaque);
        assert!(secret.validate().is_ok());
    }

    #[test]
    fn oauth_secret_validation_is_supported() {
        let secret = ProviderSecret::OAuth(OAuthMaterial {
            authorize_url: "https://example.com/auth".into(),
            token_url: "https://example.com/token".into(),
            client_id: "client-id".into(),
            client_secret: "client-secret".into(),
            redirect_uri: default_redirect_uri(),
            scopes: vec!["scope:one".into(), "scope:two".into()],
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
        });
        assert_eq!(secret.auth_method(), AuthMethod::OAuth);
        assert!(secret.validate().is_ok());
    }

    #[test]
    fn totp_matches_rfc6238_test_vector() {
        let seed = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        assert_eq!(totp_at(seed, 59, 8, 30).unwrap(), "94287082");
        assert_eq!(totp_at(seed, 1_111_111_109, 8, 30).unwrap(), "07081804");
    }

    #[test]
    fn totp_rejects_invalid_digits_and_period() {
        let seed = "JBSWY3DPEHPK3PXP";
        assert!(totp_at(seed, 60, 5, 30).is_err());
        assert!(totp_at(seed, 60, 6, 0).is_err());
    }

    #[test]
    fn otp_secret_validation_accepts_valid_seed() {
        let secret = ProviderSecret::Otp(OtpMaterial {
            seed: "JBSWY3DPEHPK3PXP".into(),
            issuer: "Passka".into(),
            account_name: "demo@example.com".into(),
            digits: 6,
            period: 30,
        });
        assert_eq!(secret.auth_method(), AuthMethod::Otp);
        assert!(secret.validate().is_ok());
    }

    #[test]
    fn otp_secret_validation_rejects_invalid_seed() {
        let secret = ProviderSecret::Otp(OtpMaterial {
            seed: "not base32!?".into(),
            issuer: String::new(),
            account_name: String::new(),
            digits: 6,
            period: 30,
        });
        assert!(secret.validate().is_err());
    }

    #[test]
    fn decode_base32_seed_rejects_empty_input() {
        assert!(decode_base32_seed("   - = ").is_err());
    }

    #[test]
    fn audit_event_kind_deserializes_legacy_values() {
        let policy_created: AuditEventKind = serde_json::from_str(r#""policy_created""#).unwrap();
        let secret_viewed: AuditEventKind = serde_json::from_str(r#""secret_viewed""#).unwrap();
        let secret_reveal_denied: AuditEventKind =
            serde_json::from_str(r#""secret_reveal_denied""#).unwrap();

        assert_eq!(policy_created, AuditEventKind::PolicyCreated);
        assert_eq!(secret_viewed, AuditEventKind::SecretViewed);
        assert_eq!(secret_reveal_denied, AuditEventKind::SecretRevealDenied);
    }
}
