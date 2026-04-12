use anyhow::{Context, Result};
use axum::body::to_bytes;
use axum::extract::Request;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use passka_core::{AccessContext, Broker, HttpRequestSpec, PrincipalKind, RegisterProviderAccount};
use serde::{Deserialize, Serialize};
use serde_json::json;
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
struct PrincipalCreateRequest {
    name: String,
    kind: PrincipalKind,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
struct AccountAuthorizeRequest {
    principal_id: String,
    #[serde(default)]
    environments: Vec<String>,
    #[serde(default = "default_lease_seconds")]
    max_lease_seconds: i64,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
struct RequestAccessRequest {
    principal_id: String,
    account_id: String,
    #[serde(default)]
    context: AccessContext,
}

#[derive(Debug, Deserialize)]
struct ProxyHttpRequest {
    lease_id: String,
    #[serde(default)]
    extra_leases: std::collections::HashMap<String, String>,
    request: HttpRequestSpec,
}

#[derive(Debug, Deserialize)]
struct CompleteAuthorizationRequest {
    code: String,
}

#[derive(Debug, Deserialize)]
struct RevealFieldRequest {
    actor_principal_id: String,
    field: String,
}

#[derive(Debug, Deserialize)]
struct AuditQuery {
    limit: Option<usize>,
}

fn default_lease_seconds() -> i64 {
    300
}

pub fn serve(addr: &str) -> Result<()> {
    let broker = Broker::new()?;
    let state = ApiState { broker };
    let app = Router::new()
        .route("/health", get(health))
        .route("/principals", get(list_principals).post(add_principal))
        .route("/accounts", get(list_accounts).post(register_account))
        .route(
            "/accounts/{account_id}",
            get(get_account).delete(remove_account),
        )
        .route("/accounts/{account_id}/authorize", post(authorize_account))
        .route("/app/accounts/{account_id}/reveal", post(reveal_account_field))
        .route("/authorizations", get(list_authorizations))
        .route("/audit", get(list_audit_events))
        .route("/access/request", post(request_access))
        .route("/http/proxy", post(proxy_http))
        .route("/oauth/{account_id}/start", post(start_authorization))
        .route("/oauth/{account_id}/complete", post(complete_authorization))
        .route("/oauth/{account_id}/refresh", post(refresh_account))
        .fallback(any(forward_proxy))
        .with_state(state);

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

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "passka-broker",
    })
}

async fn list_principals(State(state): State<ApiState>) -> ApiResult {
    json_result(state.broker.list_principals())
}

async fn add_principal(
    State(state): State<ApiState>,
    Json(request): Json<PrincipalCreateRequest>,
) -> ApiResult {
    json_result(
        state
            .broker
            .add_principal(&request.name, request.kind, &request.description),
    )
}

async fn list_accounts(State(state): State<ApiState>) -> ApiResult {
    json_result(state.broker.list_accounts())
}

async fn register_account(
    State(state): State<ApiState>,
    Json(request): Json<RegisterProviderAccount>,
) -> ApiResult {
    json_result(state.broker.register_provider_account(request))
}

async fn get_account(State(state): State<ApiState>, Path(account_id): Path<String>) -> ApiResult {
    json_result(state.broker.get_account(&account_id))
}

async fn remove_account(
    State(state): State<ApiState>,
    Path(account_id): Path<String>,
) -> ApiResult {
    match state.broker.remove_account(&account_id) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => api_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn reveal_account_field(
    State(state): State<ApiState>,
    Path(account_id): Path<String>,
    Json(request): Json<RevealFieldRequest>,
) -> ApiResult {
    json_result(state.broker.reveal_sensitive_field_for_app(
        &request.actor_principal_id,
        &account_id,
        &request.field,
    ))
}

async fn authorize_account(
    State(state): State<ApiState>,
    Path(account_id): Path<String>,
    Json(request): Json<AccountAuthorizeRequest>,
) -> ApiResult {
    json_result(state.broker.authorize_account(
        &request.principal_id,
        &account_id,
        request.environments,
        request.max_lease_seconds,
        &request.description,
    ))
}

async fn list_authorizations(State(state): State<ApiState>) -> ApiResult {
    json_result(state.broker.list_authorizations())
}

async fn list_audit_events(
    State(state): State<ApiState>,
    Query(query): Query<AuditQuery>,
) -> ApiResult {
    json_result(state.broker.list_audit_events(query.limit))
}

async fn request_access(
    State(state): State<ApiState>,
    Json(request): Json<RequestAccessRequest>,
) -> ApiResult {
    json_result(state.broker.request_access(
        &request.principal_id,
        &request.account_id,
        request.context,
    ))
}

async fn proxy_http(
    State(state): State<ApiState>,
    Json(request): Json<ProxyHttpRequest>,
) -> ApiResult {
    json_result(state.broker.proxy_http_with_leases(
        &request.lease_id,
        request.extra_leases,
        request.request,
    ))
}

async fn forward_proxy(State(state): State<ApiState>, request: Request) -> ApiResult {
    let (parts, body) = request.into_parts();
    if parts.method.as_str().eq_ignore_ascii_case("CONNECT") {
        return api_error(
            StatusCode::BAD_REQUEST,
            "HTTPS CONNECT tunnels are encrypted; Passka cannot replace tokens inside them without TLS interception".into(),
        );
    }

    let Some(lease_id) = proxy_lease_id(&parts.headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "forward proxy requests must include X-Passka-Lease or Proxy-Authorization: Bearer <lease_id>".into(),
        );
    };
    let Some(target_url) = proxy_target_url(&parts.uri, &parts.headers) else {
        return api_error(
            StatusCode::BAD_REQUEST,
            "forward proxy requests must use an absolute http(s) URL or X-Passka-Target".into(),
        );
    };

    let headers = proxy_header_map(&parts.headers);
    let extra_leases = proxy_extra_leases(&parts.headers);
    let body = match to_bytes(body, 10 * 1024 * 1024).await {
        Ok(body) => body.to_vec(),
        Err(err) => return api_error(StatusCode::BAD_REQUEST, err.to_string()),
    };
    proxy_response(state.broker.proxy_forward_http_with_leases(
        &lease_id,
        extra_leases,
        parts.method.as_str(),
        &target_url,
        headers,
        body,
    ))
}

async fn start_authorization(
    State(state): State<ApiState>,
    Path(account_id): Path<String>,
) -> ApiResult {
    json_result(state.broker.start_authorization(&account_id))
}

async fn complete_authorization(
    State(state): State<ApiState>,
    Path(account_id): Path<String>,
    Json(request): Json<CompleteAuthorizationRequest>,
) -> ApiResult {
    match state
        .broker
        .complete_authorization(&account_id, &request.code)
    {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => api_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn refresh_account(
    State(state): State<ApiState>,
    Path(account_id): Path<String>,
) -> ApiResult {
    match state.broker.refresh_account(&account_id) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => api_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

type ApiResult = Response;

fn json_result<T: Serialize>(result: Result<T>) -> ApiResult {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(err) => api_error(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

fn proxy_response(result: Result<passka_core::HttpProxyResponse>) -> ApiResult {
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

fn proxy_header_map(headers: &HeaderMap) -> std::collections::HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            let name = name.as_str();
            if name.eq_ignore_ascii_case("host") || is_hop_by_hop_header(name) {
                return None;
            }
            let value = value.to_str().ok()?;
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

fn proxy_extra_leases(headers: &HeaderMap) -> std::collections::HashMap<String, String> {
    headers
        .get("x-passka-extra-leases")
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .filter_map(|binding| {
                    let (alias, lease_id) = binding.split_once('=')?;
                    let alias = alias.trim();
                    let lease_id = lease_id.trim();
                    if alias.is_empty() || lease_id.is_empty() {
                        return None;
                    }
                    Some((alias.to_string(), lease_id.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
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
