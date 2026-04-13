use anyhow::Result;
use passka_core::Broker;

pub fn run(account_id: &str) -> Result<()> {
    let broker = Broker::new()?;
    broker.refresh_account(account_id)?;
    eprintln!("token refreshed successfully for '{account_id}'");
    Ok(())
}
