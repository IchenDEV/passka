use anyhow::{Context, Result};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use passka_core::{
    AccessContext, Broker, HttpRequestSpec, PrincipalKind, RegisterProviderAccount,
};
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
struct AllowPolicyRequest {
    principal_id: String,
    account_id: String,
    resource: String,
    actions: Vec<String>,
    #[serde(default)]
    environments: Vec<String>,
    #[serde(default = "default_lease_seconds")]
    max_lease_seconds: i64,
    #[serde(default)]
    allow_secret_reveal: bool,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
struct RequestAccessRequest {
    principal_id: String,
    resource: String,
    action: String,
    #[serde(default)]
    context: AccessContext,
}

#[derive(Debug, Deserialize)]
struct ProxyHttpRequest {
    lease_id: String,
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
        .route("/accounts/{account_id}", get(get_account).delete(remove_account))
        .route("/accounts/{account_id}/reveal", post(reveal_account_field))
        .route("/policies", get(list_policies))
        .route("/policies/allow", post(allow_policy))
        .route("/audit", get(list_audit_events))
        .route("/access/request", post(request_access))
        .route("/http/proxy", post(proxy_http))
        .route("/oauth/{account_id}/start", post(start_authorization))
        .route("/oauth/{account_id}/complete", post(complete_authorization))
        .route("/oauth/{account_id}/refresh", post(refresh_account))
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
    json_result(state.broker.add_principal(
        &request.name,
        request.kind,
        &request.description,
    ))
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

async fn get_account(
    State(state): State<ApiState>,
    Path(account_id): Path<String>,
) -> ApiResult {
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
    json_result(state.broker.reveal_sensitive_field(
        &request.actor_principal_id,
        &account_id,
        &request.field,
    ))
}

async fn list_policies(State(state): State<ApiState>) -> ApiResult {
    json_result(state.broker.list_policies())
}

async fn allow_policy(
    State(state): State<ApiState>,
    Json(request): Json<AllowPolicyRequest>,
) -> ApiResult {
    json_result(state.broker.allow_policy(
        &request.principal_id,
        &request.account_id,
        &request.resource,
        request.actions,
        request.environments,
        request.max_lease_seconds,
        request.allow_secret_reveal,
        &request.description,
    ))
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
        &request.resource,
        &request.action,
        request.context,
    ))
}

async fn proxy_http(
    State(state): State<ApiState>,
    Json(request): Json<ProxyHttpRequest>,
) -> ApiResult {
    json_result(state.broker.proxy_http(&request.lease_id, request.request))
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
    match state.broker.complete_authorization(&account_id, &request.code) {
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

fn api_error(status: StatusCode, message: String) -> ApiResult {
    (status, Json(json!({ "error": message }))).into_response()
}
