use anyhow::Result;
use passka_core::types::CredentialType;
use passka_core::IndexStore;

pub fn run(name: &str) -> Result<()> {
    let index = IndexStore::new()?;
    let meta = index.get(name)?;

    if meta.cred_type != CredentialType::Token {
        anyhow::bail!("credential '{name}' is not a token type, cannot refresh");
    }

    let rt = tokio::runtime::Runtime::new()?;
    let new_token = rt.block_on(passka_core::oauth::refresh_token(name))?;

    let masked = passka_core::types::mask_value(&new_token);
    eprintln!("token refreshed: {masked}");
    Ok(())
}
