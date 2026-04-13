use anyhow::Result;
use passka_core::{Broker, PrincipalKind};
use serde::Serialize;

#[derive(Serialize)]
struct AgentTokenIssueOutput<'a> {
    principal_id: &'a str,
    agent_token: &'a str,
}

#[derive(Serialize)]
struct AgentTokenRevokeOutput<'a> {
    ok: bool,
    principal_id: &'a str,
}

pub fn add(name: &str, kind: &str, description: Option<&str>) -> Result<()> {
    let broker = Broker::new()?;
    let kind: PrincipalKind = kind.parse().map_err(|err: String| anyhow::anyhow!(err))?;
    let principal = broker.add_principal(name, kind, description.unwrap_or_default())?;
    println!("{}", serde_json::to_string_pretty(&principal)?);
    Ok(())
}

pub fn list() -> Result<()> {
    let broker = Broker::new()?;
    let principals = broker.list_principals()?;
    println!("{}", serde_json::to_string_pretty(&principals)?);
    Ok(())
}

pub fn issue_token(principal_id: &str) -> Result<()> {
    let broker = Broker::new()?;
    let token = broker.issue_agent_token(principal_id)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&AgentTokenIssueOutput {
            principal_id,
            agent_token: &token,
        })?
    );
    Ok(())
}

pub fn revoke_token(principal_id: &str) -> Result<()> {
    let broker = Broker::new()?;
    broker.revoke_agent_token(principal_id)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&AgentTokenRevokeOutput {
            ok: true,
            principal_id,
        })?
    );
    Ok(())
}
