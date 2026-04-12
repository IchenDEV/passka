use crate::cli::{ProxyArgs, RequestArgs};
use anyhow::Result;
use passka_core::{AccessContext, Broker, HttpRequestSpec};
use std::collections::HashMap;

pub fn request(args: RequestArgs) -> Result<()> {
    let broker = Broker::new()?;
    let lease = broker.request_access(
        &args.principal,
        &args.account,
        AccessContext {
            environment: args.environment,
            purpose: args.purpose,
            source: args.source,
        },
    )?;
    println!("{}", serde_json::to_string_pretty(&lease)?);
    Ok(())
}

pub fn proxy(args: ProxyArgs) -> Result<()> {
    let broker = Broker::new()?;
    let headers = parse_headers(&args.headers)?;
    let extra_leases = parse_extra_leases(&args.extra_leases)?;
    let response = broker.proxy_http_with_leases(
        &args.lease,
        extra_leases,
        HttpRequestSpec {
            method: args.method,
            path: args.path,
            headers,
            body: args.body.unwrap_or_default(),
        },
    )?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn parse_extra_leases(values: &[String]) -> Result<HashMap<String, String>> {
    let mut leases = HashMap::new();
    for value in values {
        let (alias, lease_id) = value.split_once('=').ok_or_else(|| {
            anyhow::anyhow!("invalid extra lease '{value}', expected alias=lease_id")
        })?;
        leases.insert(alias.trim().to_string(), lease_id.trim().to_string());
    }
    Ok(leases)
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
