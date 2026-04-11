use anyhow::Result;
use passka_core::{Broker, PrincipalKind};

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
