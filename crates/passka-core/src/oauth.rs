use crate::store::keychain::KeychainStore;
use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;

pub fn needs_refresh(name: &str) -> Result<bool> {
    let data = KeychainStore::get(name)?;
    let Some(expires_at) = data.fields.get("expires_at") else {
        return Ok(false);
    };
    if expires_at.is_empty() {
        return Ok(false);
    }
    let expires =
        chrono::DateTime::parse_from_rfc3339(expires_at).context("invalid expires_at format")?;
    Ok(Utc::now() >= expires)
}

pub async fn refresh_token(name: &str) -> Result<String> {
    let data = KeychainStore::get(name)?;
    let refresh_token = data
        .fields
        .get("refresh_token")
        .filter(|v| !v.is_empty())
        .context("no refresh_token available")?;
    let refresh_url = data
        .fields
        .get("refresh_url")
        .filter(|v| !v.is_empty())
        .context("no refresh_url configured")?;

    let mut params = HashMap::new();
    params.insert("grant_type", "refresh_token");
    params.insert("refresh_token", refresh_token.as_str());

    if let Some(client_id) = data.fields.get("client_id").filter(|v| !v.is_empty()) {
        params.insert("client_id", client_id.as_str());
    }
    if let Some(client_secret) = data.fields.get("client_secret").filter(|v| !v.is_empty()) {
        params.insert("client_secret", client_secret.as_str());
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(refresh_url.as_str())
        .form(&params)
        .send()
        .await
        .context("refresh request failed")?;

    let body: serde_json::Value = resp.json().await.context("invalid refresh response")?;

    let new_token = body["access_token"]
        .as_str()
        .context("no access_token in response")?
        .to_string();

    KeychainStore::update_field(name, "token", &new_token)?;

    if let Some(expires_in) = body["expires_in"].as_i64() {
        let new_expiry = Utc::now() + chrono::Duration::seconds(expires_in);
        KeychainStore::update_field(name, "expires_at", &new_expiry.to_rfc3339())?;
    }

    if let Some(new_refresh) = body["refresh_token"].as_str() {
        KeychainStore::update_field(name, "refresh_token", new_refresh)?;
    }

    Ok(new_token)
}

pub fn get_valid_token(name: &str) -> Result<String> {
    if needs_refresh(name)? {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(refresh_token(name))
    } else {
        KeychainStore::get_field(name, "token")
    }
}
