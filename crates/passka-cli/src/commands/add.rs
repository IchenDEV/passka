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

    let fields = match cred_type {
        CredentialType::Secret => add_secret()?,
        CredentialType::OAuth => add_oauth()?,
    };

    let data = CredentialData { fields };
    KeychainStore::set(name, &data)?;

    let env_vars = CredentialMeta::default_env_vars(name, &cred_type, &data);
    let now = chrono::Utc::now().to_rfc3339();
    let meta = CredentialMeta {
        name: name.to_string(),
        cred_type: cred_type.clone(),
        description: description.unwrap_or_default().to_string(),
        env_vars,
        created_at: now.clone(),
        updated_at: now,
    };
    index.add(meta)?;

    eprintln!("credential '{name}' stored successfully");

    if cred_type == CredentialType::OAuth {
        eprintln!(
            "hint: run `passka auth {name}` to complete the OAuth authorization flow"
        );
    }

    Ok(())
}

fn add_secret() -> Result<HashMap<String, String>> {
    let mut fields = HashMap::new();

    eprintln!("enter field key-value pairs (empty key to finish):");
    loop {
        let key: String = Input::new()
            .with_prompt("  key")
            .allow_empty(true)
            .interact_text()?;
        if key.is_empty() {
            break;
        }
        let val = Password::new()
            .with_prompt(format!("  value for '{key}'"))
            .interact()?;
        fields.insert(key, val);
    }

    if fields.is_empty() {
        anyhow::bail!("secret credential requires at least one field");
    }

    Ok(fields)
}

fn add_oauth() -> Result<HashMap<String, String>> {
    let mut fields = HashMap::new();

    let authorize_url: String = Input::new()
        .with_prompt("authorize URL (required)")
        .interact_text()?;
    fields.insert("authorize_url".into(), authorize_url);

    let token_url: String = Input::new()
        .with_prompt("token URL (required)")
        .interact_text()?;
    fields.insert("token_url".into(), token_url);

    let client_id: String = Input::new()
        .with_prompt("client ID (required)")
        .interact_text()?;
    fields.insert("client_id".into(), client_id);

    let client_secret = Password::new()
        .with_prompt("client secret (required)")
        .interact()?;
    fields.insert("client_secret".into(), client_secret);

    let redirect_uri: String = Input::new()
        .with_prompt("redirect URI (default: http://localhost:8477/callback)")
        .default("http://localhost:8477/callback".into())
        .interact_text()?;
    fields.insert("redirect_uri".into(), redirect_uri);

    let scopes: String = Input::new()
        .with_prompt("scopes (space-separated, optional)")
        .allow_empty(true)
        .interact_text()?;
    if !scopes.is_empty() {
        fields.insert("scopes".into(), scopes);
    }

    Ok(fields)
}
