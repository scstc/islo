//! 把 islo 图标作为 RT_GROUP_ICON #1 编进 exe，供 LoadImageW(hInst, 1, ...) 取用。
//! 不引入额外依赖：直接调用 Windows SDK 的 rc.exe，把 .res 交给链接器。

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn find_rc() -> Option<PathBuf> {
    let roots = [
        r"C:\Program Files (x86)\Windows Kits\10\bin",
        r"C:\Program Files\Windows Kits\10\bin",
    ];
    let mut found: Vec<PathBuf> = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let rc = dir.join("x64").join("rc.exe");
            if rc.exists() {
                found.push(rc);
            }
        }
    }
    found.sort();
    found.pop()
}

fn main() {
    println!("cargo:rerun-if-changed=VERSION");
    let version = fs::read_to_string("VERSION").expect("VERSION must exist");
    let version = version.trim();
    let parts: Vec<_> = version.split('.').collect();
    assert!(parts.len() == 4 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()) && part.parse::<u16>().is_ok()), "VERSION must contain four numeric Windows version components");
    assert_eq!(parts[..3].join("."), env::var("CARGO_PKG_VERSION").unwrap(), "Cargo version must match the release branch prefix");
    println!("cargo:rustc-env=ISLO_APP_VERSION={version}");
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=assets/islo.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let directory = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out = directory.join("islo.res");
    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe 未找到，跳过图标资源");
        return;
    };
    let source = directory.join("islo.rc");
    let template = fs::read_to_string("app.rc").expect("app.rc must exist");
    fs::write(&source, format!("#define ISLO_FILE_VERSION {}\n#define ISLO_DISPLAY_VERSION \"{version}\"\n{template}", parts.join(","))).expect("write versioned resource");
    let status = Command::new(&rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&out)
        .arg("/I")
        .arg(env::var("CARGO_MANIFEST_DIR").unwrap())
        .arg(&source)
        .status();
    if !status.map(|s| s.success()).unwrap_or(false) {
        println!("cargo:warning=rc.exe 编译 app.rc 失败");
        return;
    }
    println!("cargo:rustc-link-arg={}", out.display());
}
