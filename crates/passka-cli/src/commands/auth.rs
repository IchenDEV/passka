use anyhow::{Context, Result};
use passka_core::types::CredentialType;
use passka_core::{IndexStore, KeychainStore};
use rand::Rng;
use std::sync::Arc;
use tokio::sync::oneshot;

pub fn run(name: &str) -> Result<()> {
    let index = IndexStore::new()?;
    let meta = index.get(name)?;

    if meta.cred_type != CredentialType::OAuth {
        anyhow::bail!("credential '{name}' is not oauth type");
    }

    let data = KeychainStore::get(name)?;
    let authorize_url = data
        .fields
        .get("authorize_url")
        .filter(|v| !v.is_empty())
        .context("missing authorize_url — re-add this credential")?
        .clone();
    let token_url = data
        .fields
        .get("token_url")
        .filter(|v| !v.is_empty())
        .context("missing token_url — re-add this credential")?
        .clone();
    let client_id = data
        .fields
        .get("client_id")
        .filter(|v| !v.is_empty())
        .context("missing client_id")?
        .clone();
    let client_secret = data
        .fields
        .get("client_secret")
        .filter(|v| !v.is_empty())
        .context("missing client_secret")?
        .clone();
    let redirect_uri = data
        .fields
        .get("redirect_uri")
        .cloned()
        .unwrap_or_else(|| "http://localhost:8477/callback".into());
    let scopes = data
        .fields
        .get("scopes")
        .cloned()
        .unwrap_or_default();

    let state = generate_state();

    let auth_url = build_auth_url(&authorize_url, &client_id, &redirect_uri, &scopes, &state);

    let rt = tokio::runtime::Runtime::new()?;
    let code = rt.block_on(async {
        get_authorization_code(&auth_url, &redirect_uri, &state).await
    })?;

    eprintln!("exchanging code for token...");
    let token_body = rt.block_on(async {
        passka_core::oauth::exchange_code(
            &token_url,
            &code,
            &client_id,
            &client_secret,
            &redirect_uri,
        )
        .await
    })?;

    let access_token = token_body["access_token"]
        .as_str()
        .context("no access_token in response")?;
    KeychainStore::update_field(name, "token", access_token)?;

    if let Some(refresh_token) = token_body["refresh_token"].as_str() {
        KeychainStore::update_field(name, "refresh_token", refresh_token)?;
    }

    if let Some(expires_in) = token_body["expires_in"].as_i64() {
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);
        KeychainStore::update_field(name, "expires_at", &expires_at.to_rfc3339())?;
    }

    index.update(name, |_| {})?;
    eprintln!("OAuth authorization complete for '{name}'");
    Ok(())
}

fn generate_state() -> String {
    let mut rng = rand::rng();
    (0..32)
        .map(|_| {
            let idx = rng.random_range(0..36u8);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect()
}

fn build_auth_url(
    authorize_url: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &str,
    state: &str,
) -> String {
    let mut url = url::Url::parse(authorize_url).expect("invalid authorize_url");
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("state", state);
    if !scopes.is_empty() {
        url.query_pairs_mut().append_pair("scope", scopes);
    }
    url.to_string()
}

async fn get_authorization_code(
    auth_url: &str,
    redirect_uri: &str,
    expected_state: &str,
) -> Result<String> {
    let parsed = url::Url::parse(redirect_uri).context("invalid redirect_uri")?;
    let port = parsed.port().unwrap_or(8477);
    let callback_path = parsed.path().to_string();
    let expected_state = expected_state.to_string();

    let (tx, rx) = oneshot::channel::<String>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

    let tx_clone = tx.clone();
    let state_clone = expected_state.clone();
    let path_clone = callback_path.clone();

    let app = axum::Router::new().route(
        &path_clone,
        axum::routing::get(
            move |axum::extract::Query(params): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >| {
                let tx = tx_clone.clone();
                let expected = state_clone.clone();
                async move {
                    let state = params.get("state").cloned().unwrap_or_default();
                    if state != expected {
                        return axum::response::Html(
                            "<h2>Error: state mismatch</h2>".to_string(),
                        );
                    }
                    match params.get("code") {
                        Some(code) => {
                            if let Some(sender) = tx.lock().await.take() {
                                let _ = sender.send(code.clone());
                            }
                            axum::response::Html(
                                "<h2>Authorization successful!</h2><p>You can close this tab.</p>"
                                    .to_string(),
                            )
                        }
                        None => axum::response::Html(
                            "<h2>Error: no code parameter</h2>".to_string(),
                        ),
                    }
                }
            },
        ),
    );

    let listener = match tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await {
        Ok(l) => l,
        Err(_) => {
            return fallback_manual_code(auth_url).await;
        }
    };

    eprintln!("opening browser for authorization...");
    if open::that(auth_url).is_err() {
        eprintln!("could not open browser. please visit:\n{auth_url}");
    }

    let server = axum::serve(listener, app);

    tokio::select! {
        result = rx => {
            result.context("callback channel closed")
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
            eprintln!("timeout waiting for callback, falling back to manual entry");
            fallback_manual_code(auth_url).await
        }
        _ = server => {
            anyhow::bail!("server shut down unexpectedly")
        }
    }
}

async fn fallback_manual_code(auth_url: &str) -> Result<String> {
    eprintln!("please visit this URL to authorize:\n{auth_url}\n");
    let code: String = dialoguer::Input::new()
        .with_prompt("paste the authorization code here")
        .interact_text()?;
    Ok(code)
}
