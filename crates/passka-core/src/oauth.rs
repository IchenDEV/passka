use crate::types::OAuthMaterial;
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;

pub fn needs_refresh(secret: &OAuthMaterial) -> Result<bool> {
    if secret.expires_at.is_empty() {
        return Ok(secret.access_token.is_empty() && !secret.refresh_token.is_empty());
    }
    let expires =
        chrono::DateTime::parse_from_rfc3339(&secret.expires_at).context("invalid expires_at")?;
    Ok(Utc::now() >= expires)
}

pub async fn refresh_token(secret: &OAuthMaterial) -> Result<OAuthMaterial> {
    let refresh_token = secret
        .refresh_token
        .as_str()
        .trim()
        .to_string();
    if refresh_token.is_empty() {
        anyhow::bail!("no refresh token available");
    }
    if secret.token_url.trim().is_empty() {
        anyhow::bail!("no token_url configured");
    }

    let mut params = HashMap::new();
    params.insert("grant_type", "refresh_token");
    params.insert("refresh_token", refresh_token.as_str());

    if !secret.client_id.trim().is_empty() {
        params.insert("client_id", secret.client_id.as_str());
    }
    if !secret.client_secret.trim().is_empty() {
        params.insert("client_secret", secret.client_secret.as_str());
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(secret.token_url.as_str())
        .form(&params)
        .send()
        .await
        .context("token refresh request failed")?;

    let body: Value = resp.json().await.context("invalid refresh response")?;
    apply_token_response(secret.clone(), &body)
}

pub async fn exchange_code(secret: &OAuthMaterial, code: &str) -> Result<OAuthMaterial> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", secret.client_id.as_str()),
        ("client_secret", secret.client_secret.as_str()),
        ("redirect_uri", secret.redirect_uri.as_str()),
    ];

    let client = reqwest::Client::new();
    let resp = client
        .post(&secret.token_url)
        .form(&params)
        .send()
        .await
        .context("token exchange request failed")?;

    let body: Value = resp.json().await.context("invalid token exchange response")?;
    apply_token_response(secret.clone(), &body)
}

fn apply_token_response(mut secret: OAuthMaterial, body: &Value) -> Result<OAuthMaterial> {
    secret.access_token = body["access_token"]
        .as_str()
        .context("no access_token in OAuth response")?
        .to_string();

    if let Some(refresh) = body["refresh_token"].as_str() {
        secret.refresh_token = refresh.to_string();
    }
    if let Some(expires_in) = body["expires_in"].as_i64() {
        let new_expiry = Utc::now() + chrono::Duration::seconds(expires_in);
        secret.expires_at = new_expiry.to_rfc3339();
    }
    Ok(secret)
}
