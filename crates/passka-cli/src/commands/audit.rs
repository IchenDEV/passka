use anyhow::Result;
use passka_core::Broker;

pub fn list(limit: Option<usize>) -> Result<()> {
    let broker = Broker::new()?;
    let events = broker.list_audit_events(limit)?;
    println!("{}", serde_json::to_string_pretty(&events)?);
    Ok(())
}
