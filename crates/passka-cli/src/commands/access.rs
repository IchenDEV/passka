use crate::broker_url::{configured_broker_url, default_broker_url, load_runtime};
use crate::cli::{ProxyArgs, RequestArgs};
use anyhow::{Context, Result};
use passka_core::{AccessContext, HttpRequestSpec};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn request(args: RequestArgs) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let client = reqwest::Client::new();
    let broker_url = runtime.block_on(resolve_broker_base_url(args.broker_url.as_deref(), &client))?;
    let response = runtime.block_on(async {
        let response = client
            .post(format!("{broker_url}/access/request"))
            .headers(agent_headers(&args.agent_token)?)
            .json(&json!({
                "account_id": args.account,
                "context": AccessContext {
                    environment: args.environment,
                    purpose: args.purpose,
                    source: args.source,
                }
            }))
            .send()
            .await
            .with_context(|| format!("failed to call broker daemon at {broker_url}"))?;
        response_json(response).await
    })?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

pub fn proxy(args: ProxyArgs) -> Result<()> {
    let headers = parse_headers(&args.headers)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let client = reqwest::Client::new();
    let broker_url = runtime.block_on(resolve_broker_base_url(args.broker_url.as_deref(), &client))?;
    let response = runtime.block_on(async {
        let response = client
            .post(format!("{broker_url}/http/proxy"))
            .headers(agent_headers(&args.agent_token)?)
            .json(&json!({
                "lease_id": args.lease,
                "request": HttpRequestSpec {
                    method: args.method,
                    path: args.path,
                    headers,
                    body: args.body.unwrap_or_default(),
                }
            }))
            .send()
            .await
            .with_context(|| format!("failed to call broker daemon at {broker_url}"))?;
        response_json(response).await
    })?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

async fn resolve_broker_base_url(
    explicit: Option<&str>,
    client: &reqwest::Client,
) -> Result<String> {
    if let Some(url) = configured_broker_url(explicit)? {
        return Ok(url);
    }

    if let Some(runtime) = load_runtime()? {
        if broker_health_ok(client, &runtime.url).await {
            return Ok(runtime.url);
        }
    }

    Ok(default_broker_url().to_string())
}

async fn broker_health_ok(client: &reqwest::Client, broker_url: &str) -> bool {
    match client.get(format!("{broker_url}/health")).send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

fn agent_headers(agent_token: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {agent_token}"))
            .context("invalid agent token header")?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(headers)
}

async fn response_json(response: reqwest::Response) -> Result<Value> {
    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .context("broker daemon returned a non-JSON response")?;
    if status.is_success() {
        return Ok(payload);
    }
    if let Some(message) = payload.get("error").and_then(Value::as_str) {
        anyhow::bail!(message.to_string());
    }
    anyhow::bail!("broker daemon request failed with status {status}");
}

fn parse_headers(values: &[String]) -> Result<HashMap<String, String>> {
    let mut headers = HashMap::new();
    for header in values {
        let (name, value) = header
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("invalid header '{header}', expected name:value"))?;
        headers.insert(name.trim().to_string(), value.trim().to_string());
    }
    Ok(headers)
}
