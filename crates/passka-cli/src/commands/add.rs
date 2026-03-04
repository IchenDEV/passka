use anyhow::Result;
use dialoguer::{Confirm, Input, Password};
use passka_core::types::{CredentialData, CredentialMeta, CredentialType};
use passka_core::{IndexStore, KeychainStore};
use std::collections::HashMap;

pub fn run(name: &str, type_str: &str, description: Option<&str>) -> Result<()> {
    let cred_type: CredentialType = type_str
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;
    let index = IndexStore::new()?;

    let fields = match cred_type {
        CredentialType::ApiKey => add_api_key()?,
        CredentialType::Password => add_password()?,
        CredentialType::Session => add_session()?,
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

fn add_api_key() -> Result<HashMap<String, String>> {
    let mut fields = HashMap::new();

    let key = Password::new()
        .with_prompt("API key (required)")
        .interact()?;
    fields.insert("key".into(), key);

    let has_secret = Confirm::new()
        .with_prompt("does this key have a paired secret (AK/SK)?")
        .default(false)
        .interact()?;

    if has_secret {
        let secret = Password::new()
            .with_prompt("API secret")
            .interact()?;
        fields.insert("secret".into(), secret);
    }

    let endpoint: String = Input::new()
        .with_prompt("endpoint URL (optional, enter to skip)")
        .allow_empty(true)
        .interact_text()?;
    if !endpoint.is_empty() {
        fields.insert("endpoint".into(), endpoint);
    }

    Ok(fields)
}

fn add_password() -> Result<HashMap<String, String>> {
    let mut fields = HashMap::new();

    let username: String = Input::new()
        .with_prompt("username (required)")
        .interact_text()?;
    fields.insert("username".into(), username);

    let password = Password::new()
        .with_prompt("password (required)")
        .interact()?;
    fields.insert("password".into(), password);

    let url: String = Input::new()
        .with_prompt("URL (optional, enter to skip)")
        .allow_empty(true)
        .interact_text()?;
    if !url.is_empty() {
        fields.insert("url".into(), url);
    }

    Ok(fields)
}

fn add_session() -> Result<HashMap<String, String>> {
    let mut fields = HashMap::new();

    let domain: String = Input::new()
        .with_prompt("domain (required, e.g. example.com)")
        .interact_text()?;
    fields.insert("domain".into(), domain);

    eprintln!("enter header/cookie key-value pairs (empty key to finish):");
    loop {
        let key: String = Input::new()
            .with_prompt("  header/cookie name")
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

    if fields.len() <= 1 {
        anyhow::bail!("session credential requires at least one header/cookie entry");
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
