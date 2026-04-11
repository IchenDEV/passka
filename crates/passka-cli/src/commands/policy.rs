use anyhow::Result;
use passka_core::Broker;

pub fn allow(
    principal_id: &str,
    account_id: &str,
    resource: &str,
    actions: &[String],
    environments: &[String],
    lease_seconds: i64,
    allow_secret_reveal: bool,
    description: Option<&str>,
) -> Result<()> {
    if actions.is_empty() {
        anyhow::bail!("at least one action is required");
    }
    let broker = Broker::new()?;
    let policy = broker.allow_policy(
        principal_id,
        account_id,
        resource,
        actions.to_vec(),
        environments.to_vec(),
        lease_seconds,
        allow_secret_reveal,
        description.unwrap_or_default(),
    )?;
    println!("{}", serde_json::to_string_pretty(&policy)?);
    Ok(())
}

pub fn list() -> Result<()> {
    let broker = Broker::new()?;
    let policies = broker.list_policies()?;
    println!("{}", serde_json::to_string_pretty(&policies)?);
    Ok(())
}
