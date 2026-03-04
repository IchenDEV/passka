use anyhow::{Context, Result};
use passka_core::types::CredentialType;
use passka_core::{IndexStore, KeychainStore};
use std::collections::HashMap;
use std::io::Read;
use std::process::Command;

pub fn run(names: &[String], redact: bool, command_parts: &[String]) -> Result<()> {
    let index = IndexStore::new()?;
    let mut env_map: HashMap<String, String> = HashMap::new();
    let mut sensitive_values: Vec<String> = Vec::new();

    for name in names {
        let meta = index.get(name)?;
        let data = if meta.cred_type == CredentialType::OAuth {
            let token = passka_core::oauth::get_valid_token(name)?;
            let mut d = KeychainStore::get(name)?;
            d.fields.insert("token".to_string(), token);
            d
        } else {
            KeychainStore::get(name)?
        };

        for (field, env_name) in &meta.env_vars {
            if let Some(val) = data.fields.get(field) {
                env_map.insert(env_name.clone(), val.clone());
                if redact && !val.is_empty() {
                    sensitive_values.push(val.clone());
                }
            }
        }
    }

    let (program, args) = command_parts.split_first().context("empty command")?;

    if !redact {
        let mut cmd = Command::new(program);
        cmd.args(args);
        for (env_name, val) in &env_map {
            cmd.env(env_name, val);
        }
        let status = cmd.status().context("failed to execute command")?;
        std::process::exit(status.code().unwrap_or(1));
    }

    let mut cmd = Command::new(program);
    cmd.args(args);
    for (env_name, val) in &env_map {
        cmd.env(env_name, val);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().context("failed to execute command")?;

    let mut stdout_buf = String::new();
    let mut stderr_buf = String::new();

    if let Some(mut out) = child.stdout.take() {
        out.read_to_string(&mut stdout_buf)?;
    }
    if let Some(mut err) = child.stderr.take() {
        err.read_to_string(&mut stderr_buf)?;
    }

    let status = child.wait().context("failed to wait for child process")?;

    sensitive_values.sort_by(|a, b| b.len().cmp(&a.len()));

    let redacted_stdout = redact_output(&stdout_buf, &sensitive_values);
    let redacted_stderr = redact_output(&stderr_buf, &sensitive_values);

    if !redacted_stdout.is_empty() {
        print!("{redacted_stdout}");
    }
    if !redacted_stderr.is_empty() {
        eprint!("{redacted_stderr}");
    }

    std::process::exit(status.code().unwrap_or(1));
}

fn redact_output(text: &str, sensitive_values: &[String]) -> String {
    let mut result = text.to_string();
    for val in sensitive_values {
        result = result.replace(val, "[REDACTED]");
    }
    result
}
