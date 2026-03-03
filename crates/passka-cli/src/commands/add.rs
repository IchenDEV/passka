use anyhow::Result;
use dialoguer::{Input, Password};
use passka_core::types::{CredentialData, CredentialMeta, CredentialType};
use passka_core::{IndexStore, KeychainStore};
use std::collections::HashMap;

pub fn run(name: &str, type_str: &str, description: Option<&str>) -> Result<()> {
    let cred_type: CredentialType = type_str
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;
    let index = IndexStore::new()?;

    let mut fields = HashMap::new();

    let sensitive_fields = ["password", "key", "secret_key", "access_key", "token",
        "refresh_token", "client_secret", "value"];

    for &field_name in cred_type.required_fields() {
        let val = if sensitive_fields.contains(&field_name) {
            Password::new()
                .with_prompt(format!("{field_name} (required)"))
                .interact()?
        } else {
            Input::<String>::new()
                .with_prompt(format!("{field_name} (required)"))
                .interact_text()?
        };
        fields.insert(field_name.to_string(), val);
    }

    for &field_name in cred_type.optional_fields() {
        let val = if sensitive_fields.contains(&field_name) {
            Password::new()
                .with_prompt(format!("{field_name} (optional, enter to skip)"))
                .allow_empty_password(true)
                .interact()?
        } else {
            Input::<String>::new()
                .with_prompt(format!("{field_name} (optional, enter to skip)"))
                .allow_empty(true)
                .interact_text()?
        };
        if !val.is_empty() {
            fields.insert(field_name.to_string(), val);
        }
    }

    if cred_type == CredentialType::Custom {
        loop {
            let key: String = Input::new()
                .with_prompt("field name (enter to finish)")
                .allow_empty(true)
                .interact_text()?;
            if key.is_empty() {
                break;
            }
            let val = Password::new()
                .with_prompt(format!("value for '{key}'"))
                .interact()?;
            fields.insert(key, val);
        }
    }

    let data = CredentialData { fields };
    KeychainStore::set(name, &data)?;

    let env_vars = CredentialMeta::default_env_vars(name, &cred_type);
    let now = chrono::Utc::now().to_rfc3339();
    let meta = CredentialMeta {
        name: name.to_string(),
        cred_type,
        description: description.unwrap_or_default().to_string(),
        env_vars,
        created_at: now.clone(),
        updated_at: now,
    };
    index.add(meta)?;

    eprintln!("credential '{name}' stored successfully");
    Ok(())
}
