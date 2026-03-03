use anyhow::Result;
use passka_core::IndexStore;

pub fn run_snippet(name: &str, lang: &str) -> Result<()> {
    let index = IndexStore::new()?;
    let meta = index.get(name)?;

    let fields: Vec<(&String, &String)> = meta.env_vars.iter().collect();

    match lang {
        "bash" | "sh" => {
            println!("# Recommended: inject via env vars (credential stays private)");
            println!("passka exec {name} -- your-command-here");
            println!();
            println!("# Alternative: shell substitution");
            for (field, _env) in &fields {
                println!(
                    "export {}=\"$(passka get {name} --field {field})\"",
                    meta.env_vars[*field]
                );
            }
        }
        "python" | "py" => {
            println!("import subprocess");
            println!();
            for (field, env_name) in &fields {
                println!(
                    "{} = subprocess.check_output([\"passka\", \"get\", \"{name}\", \"--field\", \"{field}\"]).decode().strip()",
                    env_name.to_lowercase()
                );
            }
        }
        "javascript" | "js" => {
            println!("const {{ execSync }} = require(\"child_process\");");
            println!();
            for (field, env_name) in &fields {
                println!(
                    "const {} = execSync(\"passka get {name} --field {field}\").toString().trim();",
                    to_camel(env_name)
                );
            }
        }
        _ => anyhow::bail!("unsupported language: {lang} (use bash, python, or javascript)"),
    }

    Ok(())
}

pub fn run_env(name: &str) -> Result<()> {
    let index = IndexStore::new()?;
    let meta = index.get(name)?;

    for (field, env_name) in &meta.env_vars {
        println!("export {env_name}=\"$(passka get {name} --field {field})\"");
    }

    Ok(())
}

fn to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.extend(ch.to_lowercase());
        }
    }
    result
}
