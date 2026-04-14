use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process;

const DEFAULT_BROKER_URL: &str = "http://127.0.0.1:8478";
pub const BROKER_URL_ENV: &str = "PASSKA_BROKER_URL";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrokerRuntime {
    pub url: String,
    pub pid: u32,
    pub started_at: String,
}

pub fn default_broker_url() -> &'static str {
    DEFAULT_BROKER_URL
}

pub fn normalize_broker_url(url: &str) -> Result<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        anyhow::bail!("broker URL cannot be empty");
    }

    let parsed = url::Url::parse(trimmed).context("invalid broker URL")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        anyhow::bail!("broker URL must use http or https");
    }
    if parsed.host_str().is_none() {
        anyhow::bail!("broker URL must include a host");
    }

    Ok(trimmed.trim_end_matches('/').to_string())
}

pub fn configured_broker_url(explicit: Option<&str>) -> Result<Option<String>> {
    if let Some(url) = explicit.map(str::trim).filter(|url| !url.is_empty()) {
        return Ok(Some(normalize_broker_url(url)?));
    }

    match std::env::var(BROKER_URL_ENV) {
        Ok(url) if !url.trim().is_empty() => Ok(Some(normalize_broker_url(&url)?)),
        Ok(_) => Ok(None),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(anyhow::Error::new(err).context(format!(
            "failed to read environment variable {BROKER_URL_ENV}"
        ))),
    }
}

pub fn broker_runtime_path() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("cannot determine config directory")?
        .join("passka")
        .join("broker")
        .join("runtime.json"))
}

pub fn load_runtime() -> Result<Option<BrokerRuntime>> {
    load_runtime_from(&broker_runtime_path()?)
}

pub fn write_runtime(url: &str) -> Result<BrokerRuntime> {
    let runtime = BrokerRuntime {
        url: normalize_broker_url(url)?,
        pid: process::id(),
        started_at: Utc::now().to_rfc3339(),
    };
    write_runtime_to(&broker_runtime_path()?, &runtime)?;
    Ok(runtime)
}

pub fn clear_runtime_if_owner(pid: u32) -> Result<()> {
    clear_runtime_from(&broker_runtime_path()?, pid)
}

fn load_runtime_from(path: &Path) -> Result<Option<BrokerRuntime>> {
    if !path.exists() {
        return Ok(None);
    }
    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read broker runtime file {}", path.display()))?;
    let runtime = serde_json::from_str(&payload)
        .with_context(|| format!("failed to decode broker runtime file {}", path.display()))?;
    Ok(Some(runtime))
}

fn write_runtime_to(path: &Path, runtime: &BrokerRuntime) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temp_path = path.with_extension(format!("{}.tmp", runtime.pid));
    fs::write(&temp_path, serde_json::to_vec_pretty(runtime)?)
        .with_context(|| format!("failed to write broker runtime file {}", temp_path.display()))?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("failed to replace broker runtime file {}", path.display()))?;
    Ok(())
}

fn clear_runtime_from(path: &Path, pid: u32) -> Result<()> {
    let Some(runtime) = load_runtime_from(path)? else {
        return Ok(());
    };
    if runtime.pid != pid {
        return Ok(());
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err)
            .with_context(|| format!("failed to remove broker runtime file {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn normalize_broker_url_trims_trailing_slashes() {
        assert_eq!(
            normalize_broker_url("http://127.0.0.1:8478/").unwrap(),
            "http://127.0.0.1:8478"
        );
    }

    #[test]
    fn normalize_broker_url_rejects_non_http_schemes() {
        assert!(normalize_broker_url("ftp://127.0.0.1:8478").is_err());
    }

    #[test]
    fn configured_broker_url_prefers_explicit_value() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var(BROKER_URL_ENV, "http://127.0.0.1:7000");

        let resolved = configured_broker_url(Some("http://127.0.0.1:9000")).unwrap();
        assert_eq!(resolved.as_deref(), Some("http://127.0.0.1:9000"));

        std::env::remove_var(BROKER_URL_ENV);
    }

    #[test]
    fn configured_broker_url_uses_environment_variable() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var(BROKER_URL_ENV, "http://127.0.0.1:7000");

        let resolved = configured_broker_url(None).unwrap();
        assert_eq!(resolved.as_deref(), Some("http://127.0.0.1:7000"));

        std::env::remove_var(BROKER_URL_ENV);
    }

    #[test]
    fn runtime_file_round_trips_and_clears_by_owner() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("runtime.json");
        let runtime = BrokerRuntime {
            url: "http://127.0.0.1:9123".into(),
            pid: 42,
            started_at: "2026-04-14T00:00:00Z".into(),
        };

        write_runtime_to(&path, &runtime).unwrap();
        assert_eq!(load_runtime_from(&path).unwrap(), Some(runtime.clone()));

        clear_runtime_from(&path, 7).unwrap();
        assert!(path.exists());

        clear_runtime_from(&path, 42).unwrap();
        assert!(!path.exists());
    }
}
