use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    UserPass,
    Cookie,
    ApiKey,
    AppSecret,
    Token,
    Custom,
}

impl CredentialType {
    pub fn required_fields(&self) -> &[&str] {
        match self {
            Self::UserPass => &["username", "password"],
            Self::Cookie => &["value", "domain"],
            Self::ApiKey => &["key"],
            Self::AppSecret => &["access_key", "secret_key"],
            Self::Token => &["token"],
            Self::Custom => &[],
        }
    }

    pub fn optional_fields(&self) -> &[&str] {
        match self {
            Self::UserPass => &["url"],
            Self::Cookie => &["path", "expires"],
            Self::ApiKey => &["provider", "endpoint"],
            Self::AppSecret => &["app_name"],
            Self::Token => &[
                "refresh_token",
                "expires_at",
                "refresh_url",
                "client_id",
                "client_secret",
            ],
            Self::Custom => &[],
        }
    }

    pub fn all_variants() -> &'static [CredentialType] {
        &[
            Self::UserPass,
            Self::Cookie,
            Self::ApiKey,
            Self::AppSecret,
            Self::Token,
            Self::Custom,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserPass => "user_pass",
            Self::Cookie => "cookie",
            Self::ApiKey => "api_key",
            Self::AppSecret => "app_secret",
            Self::Token => "token",
            Self::Custom => "custom",
        }
    }
}

impl std::fmt::Display for CredentialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for CredentialType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user_pass" => Ok(Self::UserPass),
            "cookie" => Ok(Self::Cookie),
            "api_key" => Ok(Self::ApiKey),
            "app_secret" => Ok(Self::AppSecret),
            "token" => Ok(Self::Token),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("unknown credential type: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialMeta {
    pub name: String,
    pub cred_type: CredentialType,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialData {
    pub fields: HashMap<String, String>,
}

impl CredentialMeta {
    pub fn default_env_vars(name: &str, cred_type: &CredentialType) -> HashMap<String, String> {
        let upper = name.to_uppercase().replace('-', "_");
        let mut vars = HashMap::new();
        match cred_type {
            CredentialType::ApiKey => {
                vars.insert("key".into(), format!("{upper}_API_KEY"));
            }
            CredentialType::UserPass => {
                vars.insert("username".into(), format!("{upper}_USERNAME"));
                vars.insert("password".into(), format!("{upper}_PASSWORD"));
            }
            CredentialType::AppSecret => {
                vars.insert("access_key".into(), format!("{upper}_ACCESS_KEY"));
                vars.insert("secret_key".into(), format!("{upper}_SECRET_KEY"));
            }
            CredentialType::Token => {
                vars.insert("token".into(), format!("{upper}_TOKEN"));
            }
            CredentialType::Cookie => {
                vars.insert("value".into(), format!("{upper}_COOKIE"));
            }
            CredentialType::Custom => {}
        }
        vars
    }
}

pub fn mask_value(val: &str) -> String {
    let len = val.len();
    if len <= 4 {
        return "*".repeat(len);
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
        assert_eq!(mask_value("abcde"), "ab****bcde");
    }

    #[test]
    fn test_credential_type_roundtrip() {
        for ct in CredentialType::all_variants() {
            let s = ct.as_str();
            let parsed: CredentialType = s.parse().unwrap();
            assert_eq!(*ct, parsed);
        }
    }
}
