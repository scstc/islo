//! Local settings commands. A running GUI owns mutations so stale UI state cannot overwrite CLI changes.
use crate::model::{self, Settings};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub type Patch = BTreeMap<String, String>;
const MAX_REQUEST_BYTES: u64 = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    patch: Option<Patch>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub ok: bool,
    pub version: String,
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Patch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Reply {
    fn result(result: Result<Settings, String>, backend: &str) -> Self {
        match result {
            Ok(settings) => Self {
                ok: true,
                version: env!("ISLO_APP_VERSION").into(),
                backend: backend.into(),
                settings: Some(settings.entries().into_iter().collect()),
                error: None,
            },
            Err(error) => Self {
                ok: false,
                version: env!("ISLO_APP_VERSION").into(),
                backend: backend.into(),
                settings: None,
                error: Some(error),
            },
        }
    }
}

fn changed(current: &Settings, patch: Option<&Patch>, path: &Path) -> Result<Settings, String> {
    let next = match patch {
        Some(patch) => current.patched(patch)?,
        None => current.clone(),
    };
    if patch.is_some() {
        next.save_to(path)
            .map_err(|error| format!("Cannot save settings: {error}"))?;
    }
    Ok(next)
}

pub fn offline(directory: &Path, patch: Option<&Patch>) -> Result<Reply, String> {
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("settings.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let path = directory.join("settings.txt");
    let current = match fs::read_to_string(&path) {
        Ok(text) => Settings::parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
        Err(e) => return Err(format!("Cannot read settings: {e}")),
    };
    Ok(Reply::result(changed(&current, patch, &path), "offline"))
}

pub fn execute(directory: &Path, patch: Option<Patch>, isolated: bool) -> Result<Reply, String> {
    #[cfg(windows)]
    if !isolated && crate::tray::instance_running() {
        return running(directory, Request { patch });
    }
    #[cfg(not(windows))]
    let _ = isolated;
    offline(directory, patch.as_ref())
}

#[cfg(windows)]
fn running(directory: &Path, request: Request) -> Result<Reply, String> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SERIAL: AtomicU32 = AtomicU32::new(0);
    let root = directory.join("agent-control");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let id = format!(
        "{stamp:032x}{:08x}{:08x}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    );
    let request_path = root.join(format!("req-{id}.json"));
    let response_path = root.join(format!("res-{id}.json"));
    let body = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    if body.len() as u64 > MAX_REQUEST_BYTES {
        return Err("Settings request exceeds 64 KiB".into());
    }
    model::atomic_write(&request_path, &body).map_err(|e| e.to_string())?;
    if !crate::tray::notify_agent_settings() {
        let _ = fs::remove_file(&request_path);
        return Err("The running islo did not accept agent commands. Update or restart the application; no offline overwrite was attempted.".into());
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        match fs::read(&response_path) {
            Ok(body) => {
                let reply = serde_json::from_slice(&body).map_err(|e| e.to_string());
                let _ = fs::remove_file(&response_path);
                return reply;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::thread::sleep(Duration::from_millis(25))
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let _ = fs::remove_file(&request_path);
    Err(
        "Timed out waiting for the GUI. The result is unknown; query settings before retrying."
            .into(),
    )
}

fn request_id(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let id = name.strip_prefix("req-")?.strip_suffix(".json")?;
    (id.len() == 48 && id.bytes().all(|c| c.is_ascii_hexdigit())).then(|| id.to_owned())
}

pub fn serve(app: &mut crate::app::App) -> std::io::Result<()> {
    let directory = model::config_dir();
    let root = directory.join("agent-control");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let requests: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| {
            (entry.file_type().ok()?.is_file() && request_id(&entry.path()).is_some())
                .then(|| entry.path())
        })
        .take(16)
        .collect();
    for path in requests {
        let Some(id) = request_id(&path) else {
            continue;
        };
        let result = (|| {
            if fs::metadata(&path).map_err(|e| e.to_string())?.len() > MAX_REQUEST_BYTES {
                return Err("Settings request exceeds 64 KiB".into());
            }
            let request: Request =
                serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let next = changed(
                &app.settings,
                request.patch.as_ref(),
                &directory.join("settings.txt"),
            )?;
            if request.patch.is_some() {
                app.apply_agent_settings(next.clone());
            }
            Ok(next)
        })();
        let reply = Reply::result(result, "running-app");
        model::atomic_write(
            &root.join(format!("res-{id}.json")),
            &serde_json::to_vec(&reply)?,
        )?;
        fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_patches_are_atomic_validated_and_preserve_unrelated_values() {
        let root = std::env::temp_dir().join(format!(
            "islo-cli-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut patch = Patch::new();
        patch.insert("focus_minutes".into(), "45".into());
        assert!(offline(&root, Some(&patch)).unwrap().ok);
        let path = root.join("settings.txt");
        let before = fs::read(&path).unwrap();
        patch.insert("water_minutes".into(), "9999".into());
        assert!(!offline(&root, Some(&patch)).unwrap().ok);
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "无效批量修改不能保存其中一部分"
        );
        patch.clear();
        patch.insert("language".into(), "en".into());
        assert!(offline(&root, Some(&patch)).unwrap().ok);
        let saved = Settings::load_from(&path);
        assert_eq!(saved.focus_minutes, 45);
        assert!(saved.locale().is_english());
        patch.clear();
        patch.insert("display_id".into(), "display\npet=0".into());
        assert!(
            !offline(&root, Some(&patch)).unwrap().ok,
            "CLI 不能通过值注入另一条配置"
        );
        fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(
            offline(&root, Some(&patch)).is_err(),
            "读不到现有配置时不能用默认值覆盖它"
        );
        assert_eq!(fs::read(&path).unwrap(), [0xff, 0xfe]);
        for name in ["settings.txt", "settings.lock"] {
            fs::remove_file(root.join(name)).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn settings_patch_matches_mascot_switches_and_rejects_unknown_keys() {
        let mut settings = Settings::default();
        settings.minimal = true;
        settings.pet = false;
        let mut patch = Patch::new();
        patch.insert("pet".into(), "1".into());
        let next = settings.patched(&patch).unwrap();
        assert!(next.pet && !next.minimal);
        patch.insert("minimal".into(), "1".into());
        assert!(settings.patched(&patch).is_err());
        patch.clear();
        patch.insert("does_not_exist".into(), "1".into());
        assert!(settings.patched(&patch).is_err());
    }
    #[test]
    fn concurrent_offline_agents_keep_both_changes() {
        let root = std::env::temp_dir().join(format!(
            "islo-cli-concurrent-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let handles: Vec<_> = [("focus_minutes", "45"), ("water_minutes", "60")]
            .into_iter()
            .map(|(key, value)| {
                let root = root.clone();
                std::thread::spawn(move || {
                    offline(&root, Some(&Patch::from([(key.into(), value.into())]))).unwrap()
                })
            })
            .collect();
        for handle in handles {
            assert!(handle.join().unwrap().ok);
        }
        let settings = Settings::load_from(&root.join("settings.txt"));
        assert_eq!((settings.focus_minutes, settings.water_minutes), (45, 60));
        for name in ["settings.txt", "settings.lock"] {
            fs::remove_file(root.join(name)).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }
}
