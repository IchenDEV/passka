use anyhow::Result;
use dialoguer::{Input, Password};
use passka_core::types::{OAuthMaterial, OtpMaterial};
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
        AuthMethod::Otp => {
            let seed = Password::new()
                .with_prompt("OTP seed (base32)")
                .interact()?;
            let issuer: String = Input::new()
                .with_prompt("issuer (optional)")
                .allow_empty(true)
                .interact_text()?;
            let account_name: String = Input::new()
                .with_prompt("account name (optional)")
                .allow_empty(true)
                .interact_text()?;
            let digits: u32 = Input::new()
                .with_prompt("digits")
                .default(6)
                .interact_text()?;
            let period: u64 = Input::new()
                .with_prompt("period seconds")
                .default(30)
                .interact_text()?;
            if seed.trim().is_empty() {
                anyhow::bail!("OTP seed is required");
            }
            ProviderSecret::Otp(OtpMaterial {
                seed,
                issuer,
                account_name,
                digits,
                period,
            })
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

pub fn allow(
    account_id: &str,
    principal_id: &str,
    environments: &[String],
    allowed_hosts: &[String],
    allowed_methods: &[String],
    allowed_path_prefixes: &[String],
    lease_seconds: i64,
    description: Option<&str>,
) -> Result<()> {
    let broker = Broker::new()?;
    let authorization = broker.authorize_account(
        principal_id,
        account_id,
        environments.to_vec(),
        allowed_hosts.to_vec(),
        allowed_methods.to_vec(),
        allowed_path_prefixes.to_vec(),
        lease_seconds,
        description.unwrap_or_default(),
    )?;
    println!("{}", serde_json::to_string_pretty(&authorization)?);
    Ok(())
}

pub fn show(account_id: &str) -> Result<()> {
    let broker = Broker::new()?;
    let account = broker.get_account(account_id)?;
    println!("{}", serde_json::to_string_pretty(&account)?);
    Ok(())
}

pub fn remove(account_id: &str) -> Result<()> {
    let broker = Broker::new()?;
    broker.remove_account(account_id)?;
    eprintln!("removed account '{account_id}'");
    Ok(())
}
