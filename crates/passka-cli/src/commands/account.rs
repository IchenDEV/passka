use anyhow::Result;
use dialoguer::{Input, Password};
use passka_core::types::{mask_value, OAuthMaterial};
use passka_core::{
    ApiKeyMaterial, AuthMethod, Broker, OpaqueSecretMaterial, ProviderKind, ProviderSecret,
    RegisterProviderAccount,
};
use std::collections::HashMap;

pub fn add(
    name: &str,
    provider: &str,
    auth: &str,
    base_url: Option<&str>,
    description: Option<&str>,
    scopes: &[String],
) -> Result<()> {
    let provider: ProviderKind = provider.parse().map_err(|err: String| anyhow::anyhow!(err))?;
    let auth: AuthMethod = auth.parse().map_err(|err: String| anyhow::anyhow!(err))?;
    let broker = Broker::new()?;

    let secret = match auth {
        AuthMethod::ApiKey => {
            let api_key = Password::new().with_prompt("API key").interact()?;
            let header_name: String = Input::new()
                .with_prompt("header name")
                .default("Authorization".into())
                .interact_text()?;
            let header_prefix: String = Input::new()
                .with_prompt("header prefix (empty for raw value)")
                .default("Bearer".into())
                .interact_text()?;
            let secret = Password::new()
                .with_prompt("secondary secret (optional)")
                .allow_empty_password(true)
                .interact()?;
            ProviderSecret::ApiKey(ApiKeyMaterial {
                api_key,
                header_name,
                header_prefix,
                secret,
            })
        }
        AuthMethod::OAuth => {
            let authorize_url: String = Input::new()
                .with_prompt("authorize URL")
                .interact_text()?;
            let token_url: String = Input::new().with_prompt("token URL").interact_text()?;
            let client_id: String = Input::new().with_prompt("client ID").interact_text()?;
            let client_secret = Password::new().with_prompt("client secret").interact()?;
            let redirect_uri: String = Input::new()
                .with_prompt("redirect URI")
                .default("http://localhost:8477/callback".into())
                .interact_text()?;
            let inferred_scopes = if scopes.is_empty() {
                let raw: String = Input::new()
                    .with_prompt("scopes (space-separated, optional)")
                    .allow_empty(true)
                    .interact_text()?;
                if raw.is_empty() {
                    Vec::new()
                } else {
                    raw.split_whitespace().map(str::to_string).collect()
                }
            } else {
                scopes.to_vec()
            };
            ProviderSecret::OAuth(OAuthMaterial {
                authorize_url,
                token_url,
                client_id,
                client_secret,
                redirect_uri,
                scopes: inferred_scopes,
                access_token: String::new(),
                refresh_token: String::new(),
                expires_at: String::new(),
            })
        }
        AuthMethod::Opaque => {
            eprintln!("enter opaque key-value pairs (empty key to finish):");
            let mut fields = HashMap::new();
            loop {
                let key: String = Input::new()
                    .with_prompt("  key")
                    .allow_empty(true)
                    .interact_text()?;
                if key.is_empty() {
                    break;
                }
                let value = Password::new()
                    .with_prompt(format!("  value for '{key}'"))
                    .interact()?;
                fields.insert(key, value);
            }
            if fields.is_empty() {
                anyhow::bail!("opaque account requires at least one field");
            }
            ProviderSecret::Opaque(OpaqueSecretMaterial { fields })
        }
    };

    let account = broker.register_provider_account(RegisterProviderAccount {
        name: name.to_string(),
        provider,
        base_url: base_url.unwrap_or_default().to_string(),
        description: description.unwrap_or_default().to_string(),
        scopes: scopes.to_vec(),
        secret,
    })?;

    println!("{}", serde_json::to_string_pretty(&account)?);
    Ok(())
}

pub fn list() -> Result<()> {
    let broker = Broker::new()?;
    let accounts = broker.list_accounts()?;
    println!("{}", serde_json::to_string_pretty(&accounts)?);
    Ok(())
}

pub fn show(account_id: &str) -> Result<()> {
    let broker = Broker::new()?;
    let account = broker.get_account(account_id)?;
    println!("{}", serde_json::to_string_pretty(&account)?);
    Ok(())
}

pub fn reveal(account_id: &str, field: &str, principal_id: &str, raw: bool) -> Result<()> {
    let broker = Broker::new()?;
    let value = broker.reveal_sensitive_field(principal_id, account_id, field)?;
    if raw {
        println!("{value}");
    } else {
        println!("{}", mask_value(&value));
    }
    Ok(())
}

pub fn remove(account_id: &str) -> Result<()> {
    let broker = Broker::new()?;
    broker.remove_account(account_id)?;
    eprintln!("removed account '{account_id}'");
    Ok(())
}
