//! Per-user PATH registration, owned entries only. No administrator privileges.
use crate::model::{self, Settings};
use std::path::Path;

fn normalized(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
}

fn path_change(current: &str, owned: &str, directory: &str, enabled: bool) -> (String, String) {
    let mut parts: Vec<_> = current.split(';').map(str::to_owned).collect();
    if !owned.is_empty() && (!enabled || normalized(owned) != normalized(directory)) {
        parts.retain(|p| normalized(p) != normalized(owned));
    }
    let mut owner = if enabled && normalized(owned) == normalized(directory) {
        owned.to_owned()
    } else {
        String::new()
    };
    if enabled && !parts.iter().any(|p| normalized(p) == normalized(directory)) {
        if current.is_empty() {
            parts.clear();
        }
        parts.push(directory.into());
        owner = directory.into();
    }
    (parts.join(";"), owner)
}

pub fn configure(settings: &mut Settings, enabled: bool) -> Result<(), String> {
    let directory = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Executable has no directory")?
        .to_path_buf();
    if enabled && !directory.join("islo-cli.exe").is_file() {
        return Err(
            "islo-cli.exe is missing beside islo.exe; reinstall the complete package".into(),
        );
    }
    let previous = settings.cli_enabled;
    register(&directory, enabled)?;
    let mut next = settings.clone();
    next.cli_enabled = enabled;
    if let Err(error) = next.save_to(&model::config_dir().join("settings.txt")) {
        let rollback = register(&directory, previous);
        return Err(format!(
            "Cannot save CLI preference: {error}; registration rollback: {rollback:?}"
        ));
    }
    *settings = next;
    Ok(())
}

pub fn unregister() -> Result<(), String> {
    register(Path::new(""), false)
}

pub fn refresh() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    register(
        executable.parent().ok_or("Executable has no directory")?,
        true,
    )
}

#[cfg(not(windows))]
fn register(_: &Path, _: bool) -> Result<(), String> {
    Err("System command registration requires Windows".into())
}

#[cfg(windows)]
fn register(directory: &Path, enabled: bool) -> Result<(), String> {
    use windows::{
        core::{w, PCWSTR},
        Win32::{
            Foundation::{ERROR_FILE_NOT_FOUND, LPARAM, WPARAM},
            System::Registry::*,
            UI::WindowsAndMessaging::{
                SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
            },
        },
    };
    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    unsafe fn read(key: HKEY, name: PCWSTR) -> Result<(String, REG_VALUE_TYPE), String> {
        let mut size = 0;
        let mut ty = REG_VALUE_TYPE::default();
        let rc = RegQueryValueExW(key, name, None, Some(&mut ty), None, Some(&mut size));
        if rc == ERROR_FILE_NOT_FOUND {
            return Ok((String::new(), REG_EXPAND_SZ));
        }
        if rc.0 != 0 {
            return Err(format!("Cannot read user PATH: {}", rc.0));
        }
        if ty != REG_SZ && ty != REG_EXPAND_SZ {
            return Err("User PATH has an unsupported registry type".into());
        }
        let mut bytes = vec![0; size as usize];
        let rc = RegQueryValueExW(
            key,
            name,
            None,
            Some(&mut ty),
            Some(bytes.as_mut_ptr()),
            Some(&mut size),
        );
        if rc.0 != 0 {
            return Err(format!("Cannot read user PATH: {}", rc.0));
        }
        let words: Vec<_> = bytes[..size as usize]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|c| *c != 0)
            .collect();
        Ok((String::from_utf16(&words).map_err(|e| e.to_string())?, ty))
    }
    unsafe fn write(
        key: HKEY,
        name: PCWSTR,
        value: &str,
        ty: REG_VALUE_TYPE,
    ) -> Result<(), String> {
        let words = wide(value);
        let bytes = std::slice::from_raw_parts(words.as_ptr() as *const u8, words.len() * 2);
        let rc = RegSetValueExW(key, name, 0, ty, Some(bytes));
        if rc.0 == 0 {
            Ok(())
        } else {
            Err(format!("Cannot register system command: {}", rc.0))
        }
    }
    unsafe {
        let mut raw = HKEY::default();
        let rc = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Environment"),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            None,
            &mut raw,
            None,
        );
        if rc.0 != 0 {
            return Err(format!("Cannot open user environment: {}", rc.0));
        }
        let key = Key(raw);
        let (current, ty) = read(key.0, w!("Path"))?;
        let (owned, _) = read(key.0, w!("isloCliPath"))?;
        let directory = directory
            .to_str()
            .ok_or("Installation path must be Unicode")?;
        if directory.contains([';', '\r', '\n', '\0']) {
            return Err("Installation path cannot be registered in PATH".into());
        }
        let (next, owner) = path_change(&current, &owned, directory, enabled);
        // Store ownership before PATH so a partial failure remains recoverable.
        write(key.0, w!("isloCliPath"), &owner, REG_SZ)?;
        if let Err(error) = write(key.0, w!("Path"), &next, ty) {
            let _ = write(key.0, w!("isloCliPath"), &owned, REG_SZ);
            return Err(error);
        }
        let environment = wide("Environment");
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(environment.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            100,
            Some(std::ptr::null_mut()),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_is_idempotent_and_removes_only_owned_entries() {
        let path = r"C:\tools;%JAVA_HOME%\bin";
        let dir = r"C:\Apps\islo";
        let (registered, owned) = path_change(path, "", dir, true);
        assert_eq!(registered, format!("{path};{dir}"));
        assert_eq!(
            path_change(&registered, &owned, dir, true),
            (registered.clone(), owned.clone())
        );
        assert_eq!(
            path_change(&registered, &owned, dir, false),
            (path.into(), String::new())
        );
        // A pre-existing entry belongs to the user; disabling must preserve it.
        let (unchanged, owner) = path_change(&registered, "", dir, true);
        assert!(owner.is_empty());
        assert_eq!(path_change(&unchanged, &owner, dir, false).0, registered);
        assert_eq!(
            path_change(r"C:\old;C:\tools", r"C:\old", dir, true).0,
            format!("C:\\tools;{dir}")
        );
    }
}
