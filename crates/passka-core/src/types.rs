use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    ApiKey,
    Password,
    Session,
    OAuth,
}

impl CredentialType {
    pub fn all_variants() -> &'static [CredentialType] {
        &[Self::ApiKey, Self::Password, Self::Session, Self::OAuth]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::Password => "password",
            Self::Session => "session",
            Self::OAuth => "oauth",
        }
    }

    pub fn sensitive_fields(&self) -> &[&str] {
        match self {
            Self::ApiKey => &["key", "secret"],
            Self::Password => &["password"],
            Self::Session => &[],
            Self::OAuth => &[
                "token",
                "refresh_token",
                "client_secret",
            ],
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
            "api_key" => Ok(Self::ApiKey),
            "password" => Ok(Self::Password),
            "session" => Ok(Self::Session),
            "oauth" => Ok(Self::OAuth),
            _ => Err(format!(
                "unknown type '{s}'. valid: api_key, password, session, oauth"
            )),
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
    pub fn default_env_vars(
        name: &str,
        cred_type: &CredentialType,
        data: &CredentialData,
    ) -> HashMap<String, String> {
        let upper = name.to_uppercase().replace('-', "_");
        let mut vars = HashMap::new();
        match cred_type {
            CredentialType::ApiKey => {
                vars.insert("key".into(), format!("{upper}_API_KEY"));
                if data.fields.contains_key("secret") {
                    vars.insert("secret".into(), format!("{upper}_API_SECRET"));
                }
            }
            CredentialType::Password => {
                vars.insert("username".into(), format!("{upper}_USERNAME"));
                vars.insert("password".into(), format!("{upper}_PASSWORD"));
            }
            CredentialType::Session => {
                for key in data.fields.keys() {
                    if key == "domain" || key == "expires" {
                        continue;
                    }
                    let env_suffix = key.to_uppercase().replace('-', "_");
                    vars.insert(key.clone(), format!("{upper}_{env_suffix}"));
                }
            }
            CredentialType::OAuth => {
                vars.insert("token".into(), format!("{upper}_TOKEN"));
            }
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
    }

    #[test]
    fn test_credential_type_roundtrip() {
        for ct in CredentialType::all_variants() {
            let s = ct.as_str();
            let parsed: CredentialType = s.parse().unwrap();
            assert_eq!(*ct, parsed);
        }
    }

    #[test]
    fn test_session_env_vars() {
        let mut fields = HashMap::new();
        fields.insert("Cookie".into(), "abc".into());
        fields.insert("X-CSRF-Token".into(), "xyz".into());
        let data = CredentialData { fields };
        let vars = CredentialMeta::default_env_vars("jira", &CredentialType::Session, &data);
        assert_eq!(vars.get("Cookie").unwrap(), "JIRA_COOKIE");
        assert_eq!(vars.get("X-CSRF-Token").unwrap(), "JIRA_X_CSRF_TOKEN");
    }
}
