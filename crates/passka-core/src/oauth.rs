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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::Form, routing::post, Json, Router};
    use serde_json::json;
    use std::collections::HashMap;
    use std::net::SocketAddr;

    fn sample_secret() -> OAuthMaterial {
        OAuthMaterial {
            authorize_url: "https://example.com/authorize".into(),
            token_url: "https://example.com/token".into(),
            client_id: "client-id".into(),
            client_secret: "client-secret".into(),
            redirect_uri: "http://localhost:8477/callback".into(),
            scopes: vec!["scope:one".into()],
            access_token: String::new(),
            refresh_token: "refresh-123".into(),
            expires_at: String::new(),
        }
    }

    #[test]
    fn needs_refresh_when_access_token_missing_and_refresh_present() {
        let secret = sample_secret();
        assert!(needs_refresh(&secret).unwrap());
    }

    #[test]
    fn needs_refresh_when_expired_timestamp_has_passed() {
        let mut secret = sample_secret();
        secret.access_token = "token".into();
        secret.expires_at = (Utc::now() - chrono::Duration::seconds(60)).to_rfc3339();
        assert!(needs_refresh(&secret).unwrap());
    }

    #[test]
    fn needs_refresh_rejects_invalid_timestamp() {
        let mut secret = sample_secret();
        secret.expires_at = "not-a-timestamp".into();
        assert!(needs_refresh(&secret).is_err());
    }

    #[test]
    fn apply_token_response_updates_tokens_and_expiry() {
        let secret = sample_secret();
        let body = json!({
            "access_token": "new-access",
            "refresh_token": "new-refresh",
            "expires_in": 120
        });
        let updated = apply_token_response(secret, &body).unwrap();
        assert_eq!(updated.access_token, "new-access");
        assert_eq!(updated.refresh_token, "new-refresh");
        assert!(!updated.expires_at.is_empty());
    }

    #[test]
    fn apply_token_response_requires_access_token() {
        let secret = sample_secret();
        let body = json!({ "refresh_token": "new-refresh" });
        assert!(apply_token_response(secret, &body).is_err());
    }

    #[test]
    fn refresh_token_rejects_missing_refresh_token() {
        let mut secret = sample_secret();
        secret.refresh_token.clear();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async { refresh_token(&secret).await });
        assert!(result.is_err());
    }

    #[test]
    fn refresh_token_rejects_missing_token_url() {
        let mut secret = sample_secret();
        secret.token_url.clear();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async { refresh_token(&secret).await });
        assert!(result.is_err());
    }

    #[test]
    fn refresh_token_posts_form_and_updates_secret() {
        async fn handler(Form(params): Form<HashMap<String, String>>) -> Json<Value> {
            assert_eq!(params.get("grant_type").map(String::as_str), Some("refresh_token"));
            assert_eq!(params.get("refresh_token").map(String::as_str), Some("refresh-123"));
            assert_eq!(params.get("client_id").map(String::as_str), Some("client-id"));
            assert_eq!(params.get("client_secret").map(String::as_str), Some("client-secret"));
            Json(json!({
                "access_token": "fresh-access",
                "refresh_token": "fresh-refresh",
                "expires_in": 300
            }))
        }

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let app = Router::new().route("/token", post(handler));
        let (listener, addr): (tokio::net::TcpListener, SocketAddr) = runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            (listener, addr)
        });
        runtime.spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let mut secret = sample_secret();
        secret.token_url = format!("http://{addr}/token");
        let updated = runtime.block_on(async { refresh_token(&secret).await }).unwrap();
        assert_eq!(updated.access_token, "fresh-access");
        assert_eq!(updated.refresh_token, "fresh-refresh");
        assert!(!updated.expires_at.is_empty());
    }

    #[test]
    fn exchange_code_posts_code_and_redirect_uri() {
        async fn handler(Form(params): Form<HashMap<String, String>>) -> Json<Value> {
            assert_eq!(
                params.get("grant_type").map(String::as_str),
                Some("authorization_code")
            );
            assert_eq!(params.get("code").map(String::as_str), Some("code-123"));
            assert_eq!(
                params.get("redirect_uri").map(String::as_str),
                Some("http://localhost:8477/callback")
            );
            Json(json!({
                "access_token": "issued-access",
                "refresh_token": "issued-refresh",
                "expires_in": 180
            }))
        }

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let app = Router::new().route("/token", post(handler));
        let (listener, addr): (tokio::net::TcpListener, SocketAddr) = runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            (listener, addr)
        });
        runtime.spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let mut secret = sample_secret();
        secret.token_url = format!("http://{addr}/token");
        let updated = runtime
            .block_on(async { exchange_code(&secret, "code-123").await })
            .unwrap();
        assert_eq!(updated.access_token, "issued-access");
        assert_eq!(updated.refresh_token, "issued-refresh");
        assert!(!updated.expires_at.is_empty());
    }
}
