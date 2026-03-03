use passka_core::types::{CredentialData, CredentialMeta, CredentialType};
use passka_core::{IndexStore, KeychainStore};
use std::collections::HashMap;

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        fn passka_list_credentials(type_filter: Option<String>) -> String;
        fn passka_get_credential_meta(name: &str) -> Option<String>;
        fn passka_get_credential_value(name: &str, field: &str) -> Option<String>;
        fn passka_add_credential(
            name: &str,
            cred_type: &str,
            data_json: &str,
            description: &str,
        ) -> String;
        fn passka_update_credential(name: &str, field: &str, value: &str) -> String;
        fn passka_remove_credential(name: &str) -> String;
        fn passka_refresh_token(name: &str) -> String;
    }
}

fn passka_list_credentials(type_filter: Option<String>) -> String {
    let index = match IndexStore::new() {
        Ok(i) => i,
        Err(e) => return json_error(&e.to_string()),
    };
    let filter = type_filter.and_then(|s| s.parse::<CredentialType>().ok());
    match index.list(filter.as_ref()) {
        Ok(entries) => serde_json::to_string(&entries).unwrap_or_else(|e| json_error(&e.to_string())),
        Err(e) => json_error(&e.to_string()),
    }
}

fn passka_get_credential_meta(name: &str) -> Option<String> {
    let index = IndexStore::new().ok()?;
    let meta = index.get(name).ok()?;
    serde_json::to_string(&meta).ok()
}

fn passka_get_credential_value(name: &str, field: &str) -> Option<String> {
    if field == "token" {
        if let Ok(index) = IndexStore::new() {
            if let Ok(meta) = index.get(name) {
                if meta.cred_type == CredentialType::Token {
                    return passka_core::oauth::get_valid_token(name).ok();
                }
            }
        }
    }
    KeychainStore::get_field(name, field).ok()
}

fn passka_add_credential(
    name: &str,
    cred_type: &str,
    data_json: &str,
    description: &str,
) -> String {
    let ct: CredentialType = match cred_type.parse() {
        Ok(t) => t,
        Err(e) => return json_error(&e),
    };
    let fields: HashMap<String, String> = match serde_json::from_str(data_json) {
        Ok(f) => f,
        Err(e) => return json_error(&e.to_string()),
    };
    let data = CredentialData { fields };
    if let Err(e) = KeychainStore::set(name, &data) {
        return json_error(&e.to_string());
    }
    let now = chrono::Utc::now().to_rfc3339();
    let meta = CredentialMeta {
        name: name.to_string(),
        cred_type: ct.clone(),
        description: description.to_string(),
        env_vars: CredentialMeta::default_env_vars(name, &ct),
        created_at: now.clone(),
        updated_at: now,
    };
    let index = match IndexStore::new() {
        Ok(i) => i,
        Err(e) => return json_error(&e.to_string()),
    };
    match index.add(meta) {
        Ok(()) => r#"{"ok":true}"#.to_string(),
        Err(e) => json_error(&e.to_string()),
    }
}

fn passka_update_credential(name: &str, field: &str, value: &str) -> String {
    match KeychainStore::update_field(name, field, value) {
        Ok(()) => {
            if let Ok(index) = IndexStore::new() {
                let _ = index.update(name, |_| {});
            }
            r#"{"ok":true}"#.to_string()
        }
        Err(e) => json_error(&e.to_string()),
    }
}

fn passka_remove_credential(name: &str) -> String {
    if let Err(e) = KeychainStore::delete(name) {
        return json_error(&e.to_string());
    }
    let index = match IndexStore::new() {
        Ok(i) => i,
        Err(e) => return json_error(&e.to_string()),
    };
    match index.remove(name) {
        Ok(()) => r#"{"ok":true}"#.to_string(),
        Err(e) => json_error(&e.to_string()),
    }
}

fn passka_refresh_token(name: &str) -> String {
    match passka_core::oauth::get_valid_token(name) {
        Ok(token) => {
            let masked = passka_core::types::mask_value(&token);
            format!(r#"{{"ok":true,"masked":"{masked}"}}"#)
        }
        Err(e) => json_error(&e.to_string()),
    }
}

fn json_error(msg: &str) -> String {
    let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
    format!(r#"{{"error":"{escaped}"}}"#)
}
