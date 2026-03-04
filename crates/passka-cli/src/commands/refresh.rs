use anyhow::Result;
use passka_core::types::CredentialType;
use passka_core::IndexStore;

pub fn run(name: &str) -> Result<()> {
    let index = IndexStore::new()?;
    let meta = index.get(name)?;

    if meta.cred_type != CredentialType::OAuth {
        anyhow::bail!("credential '{name}' is not oauth type, cannot refresh");
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(passka_core::oauth::refresh_token(name))?;

    eprintln!("token refreshed successfully for '{name}'");
    Ok(())
}
