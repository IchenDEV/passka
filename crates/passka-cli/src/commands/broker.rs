use anyhow::{Context, Result};
use axum::body::to_bytes;
use axum::extract::Request;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use passka_core::{AccessContext, Broker, HttpRequestSpec, HttpProxyResponse, Principal};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;

#[derive(Clone)]
struct ApiState {
    broker: Broker,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    service: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestAccessRequest {
    account_id: String,
    #[serde(default)]
    context: AccessContext,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProxyHttpRequest {
    lease_id: String,
    request: HttpRequestSpec,
}

pub fn serve(addr: &str) -> Result<()> {
    let broker = Broker::new()?;
    let state = ApiState { broker };
    let app = app_router(state);

    let addr: SocketAddr = addr.parse().context("invalid broker listen address")?;
    eprintln!("passka broker listening on http://{addr}");
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .context("failed to bind broker listener")?;
        axum::serve(listener, app)
            .await
            .context("broker server failed")
    })
}

fn app_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/access/request", post(request_access))
        .route("/http/proxy", post(proxy_http))
        .fallback(any(forward_proxy))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "passka-broker",
    })
}

async fn request_access(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<RequestAccessRequest>,
) -> ApiResult {
    let principal = match authenticate_json_agent(&state.broker, &headers) {
        Ok(principal) => principal,
        Err(err) => return api_error(StatusCode::UNAUTHORIZED, err.to_string()),
    };
    json_result(state.broker.request_access(
        &principal.id,
        &request.account_id,
        request.context,
    ))
}

async fn proxy_http(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<ProxyHttpRequest>,
) -> ApiResult {
    let principal = match authenticate_json_agent(&state.broker, &headers) {
        Ok(principal) => principal,
        Err(err) => return api_error(StatusCode::UNAUTHORIZED, err.to_string()),
    };
    proxy_response(
        state
            .broker
            .proxy_http_async(&principal.id, &request.lease_id, request.request)
            .await,
    )
}

async fn forward_proxy(State(state): State<ApiState>, request: Request) -> ApiResult {
    let (parts, body) = request.into_parts();
    if parts.method.as_str().eq_ignore_ascii_case("CONNECT") {
        return api_error(
            StatusCode::BAD_REQUEST,
            "HTTPS CONNECT tunnels are encrypted; Passka cannot inspect or inject headers inside them without TLS interception".into(),
        );
    }

    let Some(target_url) = proxy_target_url(&parts.uri, &parts.headers) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let principal = match authenticate_forward_proxy_agent(&state.broker, &parts.headers) {
        Ok(principal) => principal,
        Err(err) => return api_error(StatusCode::UNAUTHORIZED, err.to_string()),
    };
    let Some(lease_id) = proxy_lease_id(&parts.headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "forward proxy requests must include X-Passka-Lease or Proxy-Authorization: Bearer <lease_id>".into(),
        );
    };

    let headers = proxy_header_map(&parts.headers);
    let body = match to_bytes(body, 10 * 1024 * 1024).await {
        Ok(body) => body.to_vec(),
        Err(err) => return api_error(StatusCode::BAD_REQUEST, err.to_string()),
    };
    proxy_response(
        state
            .broker
            .proxy_forward_http_async(
                &principal.id,
                &lease_id,
                parts.method.as_str(),
                &target_url,
                headers,
                body,
            )
            .await,
    )
}

type ApiResult = Response;

fn json_result<T: Serialize>(result: Result<T>) -> ApiResult {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(err) => api_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

fn proxy_response(result: Result<HttpProxyResponse>) -> ApiResult {
    match result {
        Ok(value) => {
            let status = StatusCode::from_u16(value.status).unwrap_or(StatusCode::BAD_GATEWAY);
            let mut response = (status, value.body).into_response();
            for (name, value) in value.headers {
                if is_hop_by_hop_header(&name) {
                    continue;
                }
                let Ok(name) = HeaderName::try_from(name.as_str()) else {
                    continue;
                };
                let Ok(value) = HeaderValue::try_from(value.as_str()) else {
                    continue;
                };
                response.headers_mut().insert(name, value);
            }
            response
        }
        Err(err) => api_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

fn api_error(status: StatusCode, message: String) -> ApiResult {
    (status, Json(json!({ "error": message }))).into_response()
}

fn authenticate_json_agent(broker: &Broker, headers: &HeaderMap) -> Result<Principal> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing Authorization: Bearer <agent_token>"))?;
    broker.authenticate_agent_token(token)
}

fn authenticate_forward_proxy_agent(broker: &Broker, headers: &HeaderMap) -> Result<Principal> {
    let token = headers
        .get("x-passka-agent-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing X-Passka-Agent-Token header"))?;
    broker.authenticate_agent_token(token)
}

fn proxy_lease_id(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get("x-passka-lease")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }

    let value = headers.get("proxy-authorization")?.to_str().ok()?.trim();
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn proxy_target_url(uri: &axum::http::Uri, headers: &HeaderMap) -> Option<String> {
    if uri.scheme().is_some() && uri.authority().is_some() {
        return Some(uri.to_string());
    }
    headers
        .get("x-passka-target")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(ToString::to_string)
}

fn proxy_header_map(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            let name = name.as_str();
            if name.eq_ignore_ascii_case("host")
                || name.eq_ignore_ascii_case("x-passka-agent-token")
                || is_hop_by_hop_header(name)
            {
                return None;
            }
            let value = value.to_str().ok()?;
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, http::HeaderMap, routing::get, Router};
    use passka_core::{ApiKeyMaterial, ProviderKind, ProviderSecret, RegisterProviderAccount};
    use serde_json::Value;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tower::ServiceExt;

    struct TestEnv {
        _temp: Arc<TempDir>,
        broker: Broker,
        token: String,
    }

    fn with_env() -> TestEnv {
        let temp = Arc::new(tempfile::tempdir().unwrap());
        let broker = Broker::from_dir(temp.path().join("passka").join("broker")).unwrap();
        let token = broker.issue_agent_token("principal:local-agent").unwrap();
        TestEnv {
            _temp: temp,
            broker,
            token,
        }
    }

    fn router(env: &TestEnv) -> Router {
        app_router(ApiState {
            broker: env.broker.clone(),
        })
    }

    async fn json_response(response: Response) -> Value {
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn daemon_rejects_missing_agent_token() {
        let env = with_env();
        let response = router(&env)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/access/request")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        r#"{"account_id":"account-1","context":{}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn daemon_rejects_unknown_admin_route() {
        let env = with_env();
        let response = router(&env)
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/accounts")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn access_request_uses_token_principal_and_rejects_old_body_shape() {
        let env = with_env();
        let account = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "openai".into(),
                provider: ProviderKind::OpenAI,
                base_url: "https://api.openai.com".into(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "sk-test".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        env.broker
            .authorize_account(
                "principal:local-agent",
                &account.id,
                vec![],
                vec![],
                vec![],
                vec![],
                300,
                "",
            )
            .unwrap();

        let response = router(&env)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/access/request")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", env.token))
                    .body(axum::body::Body::from(format!(
                        r#"{{"principal_id":"principal:local-human","account_id":"{}","context":{{}}}}"#,
                        account.id
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn proxy_rejects_token_principal_mismatch() {
        let env = with_env();
        let account = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "github".into(),
                provider: ProviderKind::GitHub,
                base_url: String::new(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "abc123".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        env.broker
            .authorize_account(
                "principal:local-agent",
                &account.id,
                vec![],
                vec![],
                vec![],
                vec![],
                300,
                "",
            )
            .unwrap();
        let lease = env
            .broker
            .request_access(
                "principal:local-agent",
                &account.id,
                AccessContext::default(),
            )
            .unwrap();

        let human_token = {
            let result = env.broker.issue_agent_token("principal:local-human");
            assert!(result.is_err());
            let other_agent = env
                .broker
                .add_principal("other-agent", passka_core::PrincipalKind::Agent, "")
                .unwrap();
            env.broker.issue_agent_token(&other_agent.id).unwrap()
        };

        let response = router(&env)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/http/proxy")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", human_token))
                    .body(axum::body::Body::from(format!(
                        r#"{{"lease_id":"{}","request":{{"method":"GET","path":"https://example.com","headers":{{}},"body":""}}}}"#,
                        lease.id
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = json_response(response).await;
        assert!(payload["error"]
            .as_str()
            .unwrap()
            .contains("belongs to principal"));
    }

    #[tokio::test]
    async fn forward_proxy_requires_x_passka_agent_token() {
        let env = with_env();
        let response = router(&env)
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("https://example.com/v1/models")
                    .header("x-passka-lease", "lease-1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn forward_proxy_uses_token_authenticated_principal() {
        let env = with_env();
        let primary = env
            .broker
            .register_provider_account(RegisterProviderAccount {
                name: "openai".into(),
                provider: ProviderKind::OpenAI,
                base_url: String::new(),
                description: String::new(),
                scopes: vec![],
                secret: ProviderSecret::ApiKey(ApiKeyMaterial {
                    api_key: "sk-primary".into(),
                    header_name: "Authorization".into(),
                    header_prefix: "Bearer".into(),
                    secret: String::new(),
                }),
            })
            .unwrap();
        env.broker
            .authorize_account(
                "principal:local-agent",
                &primary.id,
                vec![],
                vec!["127.0.0.1".into()],
                vec![],
                vec![],
                300,
                "",
            )
            .unwrap();
        let lease = env
            .broker
            .request_access(
                "principal:local-agent",
                &primary.id,
                AccessContext::default(),
            )
            .unwrap();

        async fn handler(headers: HeaderMap, State(expected): State<String>) -> String {
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .filter(|value| *value == format!("Bearer {expected}"))
                .map(|_| "ok".to_string())
                .unwrap_or_else(|| "missing".to_string())
        }

        let app = Router::new()
            .route("/v1/models", get(handler))
            .with_state("sk-primary".to_string());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let response = router(&env)
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("http://{addr}/v1/models"))
                    .header("x-passka-agent-token", &env.token)
                    .header("x-passka-lease", lease.id)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(String::from_utf8(body.to_vec()).unwrap(), "ok");
    }
}
