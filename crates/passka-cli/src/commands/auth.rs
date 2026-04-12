use anyhow::{Context, Result};
use dialoguer::Input;
use passka_core::Broker;
use rand::Rng;
use std::sync::Arc;
use tokio::sync::oneshot;

pub fn run(account_id: &str) -> Result<()> {
    let broker = Broker::new()?;
    let session = broker.start_authorization(account_id)?;
    let redirect_uri = broker
        .read_account_field_internal(account_id, "redirect_uri")
        .unwrap_or_else(|_| "http://localhost:8477/callback".into());
    let state = generate_state();
    let auth_url = append_state(&session.authorization_url, &state)?;

    let runtime = tokio::runtime::Runtime::new()?;
    let code = runtime.block_on(async { get_authorization_code(&auth_url, &redirect_uri, &state).await })?;
    broker.complete_authorization(account_id, &code)?;
    eprintln!("OAuth authorization complete for '{account_id}'");
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

fn append_state(auth_url: &str, state: &str) -> Result<String> {
    let mut url = url::Url::parse(auth_url).context("invalid authorization URL")?;
    url.query_pairs_mut().append_pair("state", state);
    Ok(url.to_string())
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
                        return axum::response::Html("<h2>Error: state mismatch</h2>".to_string());
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
                        None => {
                            axum::response::Html("<h2>Error: no code parameter</h2>".to_string())
                        }
                    }
                }
            },
        ),
    );

    let listener = match tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await {
        Ok(listener) => listener,
        Err(_) => return fallback_manual_code(auth_url).await,
    };

    eprintln!("opening browser for authorization...");
    if open::that(auth_url).is_err() {
        eprintln!("could not open browser. please visit:\n{auth_url}");
    }

    let server = axum::serve(listener, app);

    tokio::select! {
        result = rx => result.context("callback channel closed"),
        _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
            eprintln!("timeout waiting for callback, falling back to manual entry");
            fallback_manual_code(auth_url).await
        }
        _ = server => anyhow::bail!("server shut down unexpectedly"),
    }
}

async fn fallback_manual_code(auth_url: &str) -> Result<String> {
    eprintln!("please visit this URL to authorize:\n{auth_url}\n");
    let code: String = Input::new()
        .with_prompt("paste the authorization code here")
        .interact_text()?;
    Ok(code)
}
