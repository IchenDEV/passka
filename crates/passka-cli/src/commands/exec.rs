use anyhow::{Context, Result};
use passka_core::types::CredentialType;
use passka_core::{IndexStore, KeychainStore};
use std::process::Command;

pub fn run(name: &str, command_parts: &[String]) -> Result<()> {
    let index = IndexStore::new()?;
    let meta = index.get(name)?;

    let data = if meta.cred_type == CredentialType::Token {
        let token = passka_core::oauth::get_valid_token(name)?;
        let mut d = KeychainStore::get(name)?;
        d.fields.insert("token".to_string(), token);
        d
    } else {
        KeychainStore::get(name)?
    };

    let (program, args) = command_parts
        .split_first()
        .context("empty command")?;

    let mut cmd = Command::new(program);
    cmd.args(args);

    for (field, env_name) in &meta.env_vars {
        if let Some(val) = data.fields.get(field) {
            cmd.env(env_name, val);
        }
    }

    let status = cmd.status().context("failed to execute command")?;

    std::process::exit(status.code().unwrap_or(1));
}
