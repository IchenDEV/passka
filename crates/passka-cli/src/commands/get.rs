use anyhow::Result;
use passka_core::types::CredentialType;
use passka_core::{IndexStore, KeychainStore};

pub fn run(name: &str, field: Option<&str>) -> Result<()> {
    let index = IndexStore::new()?;
    let meta = index.get(name)?;

    if meta.cred_type == CredentialType::Token {
        if field.is_none() || field == Some("token") {
            let token = passka_core::oauth::get_valid_token(name)?;
            print!("{token}");
            return Ok(());
        }
    }

    match field {
        Some(f) => {
            let val = KeychainStore::get_field(name, f)?;
            print!("{val}");
        }
        None => {
            let primary = meta.cred_type.required_fields().first().copied()
                .unwrap_or("value");
            let val = KeychainStore::get_field(name, primary)?;
            print!("{val}");
        }
    }
    Ok(())
}
