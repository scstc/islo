//! CLI entry point. Settings behavior lives in the shared islo library.
use islo::{
    agent_control::{self, Patch},
    model,
};
use std::path::PathBuf;

const USAGE:&str="islo-cli [--data-dir ISOLATED_DIR] settings list | settings get KEY | settings set KEY VALUE | settings apply --file PATCH.json. Values use settings.txt format; booleans: 0/1.";

fn run() -> Result<serde_json::Value, String> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let mut directory = model::config_dir();
    let mut isolated = false;
    if args.first().is_some_and(|v| v == "--data-dir") {
        if args.len() < 3 {
            return Err("--data-dir requires a directory and command".into());
        }
        let default_directory = directory.clone();
        directory = PathBuf::from(args.remove(1));
        args.remove(0);
        isolated = directory
            .canonicalize()
            .ok()
            .zip(default_directory.canonicalize().ok())
            .is_none_or(|(a, b)| a != b);
    }
    if args.is_empty() || args == ["--help"] {
        return Ok(serde_json::json!({"ok":true,"usage":USAGE,"version":env!("ISLO_APP_VERSION")}));
    }
    if args == ["--version"] {
        return Ok(serde_json::json!({"ok":true,"version":env!("ISLO_APP_VERSION")}));
    }
    let (patch, key) = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["settings", "list"] => (None, None),
        ["settings", "get", key] => (None, Some((*key).to_owned())),
        ["settings", "set", key, value] => (
            Some(Patch::from([((*key).to_owned(), (*value).to_owned())])),
            None,
        ),
        ["settings", "apply", "--file", path] => {
            let file = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            (
                Some(
                    serde_json::from_str::<Patch>(file.trim_start_matches('\u{feff}')).map_err(
                        |e| format!("Patch must be a JSON object with string values: {e}"),
                    )?,
                ),
                None,
            )
        }
        _ => return Err(USAGE.into()),
    };
    let reply = agent_control::execute(&directory, patch, isolated)?;
    if !reply.ok {
        return Err(reply
            .error
            .unwrap_or_else(|| "Settings request failed".into()));
    }
    let mut result = serde_json::to_value(reply).map_err(|e| e.to_string())?;
    if let Some(key) = key {
        let value = result["settings"]
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("Unknown setting: {key}"))?;
        result.as_object_mut().unwrap().remove("settings");
        result["key"] = key.into();
        result["value"] = value;
    }
    Ok(result)
}

fn main() {
    match run() {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("{}", serde_json::json!({"ok":false,"error":error}));
            std::process::exit(1);
        }
    }
}
