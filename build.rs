//! Nhúng icon của app (assets/icon.ico) vào file exe, để Windows (taskbar, lối tắt, Task Manager)
//! hiện đúng icon. Cần rc.exe của Windows SDK; không có thì bỏ qua, exe vẫn build được.

use std::path::PathBuf;

fn find_rc() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into())).join(r"Windows Kits\10\bin");
    let mut vers: Vec<PathBuf> = std::fs::read_dir(&root).ok()?.flatten().map(|e| e.path()).filter(|p| p.join(r"x64\rc.exe").exists()).collect();
    vers.sort();
    vers.pop().map(|p| p.join(r"x64\rc.exe"))
}

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join(r"assets\icon.ico");
    if !ico.exists() {
        println!("cargo:warning=không có assets\\icon.ico, exe sẽ không có icon");
        return;
    }
    let Some(rc) = find_rc() else {
        println!("cargo:warning=không thấy rc.exe (Windows SDK), exe sẽ không có icon");
        return;
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let rc_file = out.join("app.rc");
    let res_file = out.join("app.res");
    let _ = std::fs::write(&rc_file, format!("1 ICON \"{}\"\n", ico.display().to_string().replace('\\', "\\\\")));
    let ok = std::process::Command::new(rc).arg("/nologo").arg("/fo").arg(&res_file).arg(&rc_file).status().map(|s| s.success()).unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bins={}", res_file.display());
    } else {
        println!("cargo:warning=rc.exe lỗi, exe sẽ không có icon");
    }
}
