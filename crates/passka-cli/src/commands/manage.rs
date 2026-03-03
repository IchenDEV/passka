use anyhow::Result;
use dialoguer::{Confirm, Password};
use passka_core::{IndexStore, KeychainStore};

pub fn run_rm(name: &str) -> Result<()> {
    let confirmed = Confirm::new()
        .with_prompt(format!("delete credential '{name}'?"))
        .default(false)
        .interact()?;

    if !confirmed {
        eprintln!("cancelled");
        return Ok(());
    }

    let index = IndexStore::new()?;
    KeychainStore::delete(name)?;
    index.remove(name)?;
    eprintln!("credential '{name}' deleted");
    Ok(())
}

pub fn run_update(name: &str, field: &str) -> Result<()> {
    let index = IndexStore::new()?;
    let _ = index.get(name)?;

    let new_value = Password::new()
        .with_prompt(format!("new value for '{field}'"))
        .interact()?;

    KeychainStore::update_field(name, field, &new_value)?;

    index.update(name, |_meta| {})?;
    eprintln!("field '{field}' updated for '{name}'");
    Ok(())
}
