use crate::types::{CredentialMeta, CredentialType};
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

pub struct IndexStore {
    path: PathBuf,
}

impl IndexStore {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("cannot determine config directory")?
            .join("passka");
        fs::create_dir_all(&config_dir)?;
        Ok(Self {
            path: config_dir.join("index.json"),
        })
    }

    pub fn load(&self) -> Result<Vec<CredentialMeta>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.path)?;
        let entries: Vec<CredentialMeta> = serde_json::from_str(&content)?;
        Ok(entries)
    }

    fn save(&self, entries: &[CredentialMeta]) -> Result<()> {
        let json = serde_json::to_string_pretty(entries)?;
        fs::write(&self.path, json)?;
        Ok(())
    }

    pub fn add(&self, meta: CredentialMeta) -> Result<()> {
        let mut entries = self.load()?;
        if entries.iter().any(|e| e.name == meta.name) {
            anyhow::bail!("credential '{}' already exists", meta.name);
        }
        entries.push(meta);
        self.save(&entries)
    }

    pub fn get(&self, name: &str) -> Result<CredentialMeta> {
        let entries = self.load()?;
        entries
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| anyhow::anyhow!("credential '{name}' not found"))
    }

    pub fn update(&self, name: &str, f: impl FnOnce(&mut CredentialMeta)) -> Result<()> {
        let mut entries = self.load()?;
        let entry = entries
            .iter_mut()
            .find(|e| e.name == name)
            .ok_or_else(|| anyhow::anyhow!("credential '{name}' not found"))?;
        f(entry);
        entry.updated_at = chrono::Utc::now().to_rfc3339();
        self.save(&entries)
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        let mut entries = self.load()?;
        let len_before = entries.len();
        entries.retain(|e| e.name != name);
        if entries.len() == len_before {
            anyhow::bail!("credential '{name}' not found");
        }
        self.save(&entries)
    }

    pub fn list(&self, type_filter: Option<&CredentialType>) -> Result<Vec<CredentialMeta>> {
        let entries = self.load()?;
        match type_filter {
            Some(ct) => Ok(entries.into_iter().filter(|e| e.cred_type == *ct).collect()),
            None => Ok(entries),
        }
    }
}
