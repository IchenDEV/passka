use anyhow::Result;
use passka_core::types::{mask_value, CredentialType};
use passka_core::{IndexStore, KeychainStore};

pub fn run_list(type_filter: Option<&str>) -> Result<()> {
    let index = IndexStore::new()?;
    let filter = type_filter
        .map(|s| s.parse::<CredentialType>().map_err(|e| anyhow::anyhow!(e)))
        .transpose()?;
    let entries = index.list(filter.as_ref())?;

    if entries.is_empty() {
        eprintln!("no credentials stored");
        return Ok(());
    }

    let max_name = entries.iter().map(|e| e.name.len()).max().unwrap_or(10);
    let max_type = entries
        .iter()
        .map(|e| e.cred_type.as_str().len())
        .max()
        .unwrap_or(10);

    println!(
        "{:<width_n$}  {:<width_t$}  {}",
        "NAME",
        "TYPE",
        "DESCRIPTION",
        width_n = max_name,
        width_t = max_type,
    );

    for entry in &entries {
        println!(
            "{:<width_n$}  {:<width_t$}  {}",
            entry.name,
            entry.cred_type.as_str(),
            entry.description,
            width_n = max_name,
            width_t = max_type,
        );
    }

    Ok(())
}

pub fn run_show(name: &str) -> Result<()> {
    let index = IndexStore::new()?;
    let meta = index.get(name)?;
    let data = KeychainStore::get(name)?;

    println!("Name:        {}", meta.name);
    println!("Type:        {}", meta.cred_type);
    println!("Description: {}", meta.description);
    println!("Created:     {}", meta.created_at);
    println!("Updated:     {}", meta.updated_at);
    println!();

    let sensitive = meta.cred_type.sensitive_fields();
    println!("Fields:");
    for (field, val) in &data.fields {
        let display = if sensitive.contains(&field.as_str()) {
            mask_value(val)
        } else {
            val.clone()
        };
        println!("  {field:<20} {display}");
    }

    println!();
    println!("Env vars:");
    for (field, env_name) in &meta.env_vars {
        println!("  {field:<20} -> ${env_name}");
    }

    Ok(())
}
