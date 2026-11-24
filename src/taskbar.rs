//! (Cách cũ) Tô màu taskbar bằng màu nhấn của Windows. Nay chỉ còn dùng để KHÔI PHỤC
//! màu gốc cho máy từng áp dụng cách này; taskbar mới nằm ở src/bar.
//! Tô màu taskbar theo chủ đề bằng chính tuỳ chọn của Windows
//! ("Hiện màu nhấn trên Start và taskbar" + màu nhấn).
//!
//! An toàn: trước lần áp dụng đầu tiên, các giá trị gốc được sao lưu vào
//! %APPDATA%\season-desktop\taskbar-backup.txt; "Khôi phục" ghi lại đúng các giá trị đó
//! (giá trị nào lúc đầu không có thì xoá đi). Chỉ đụng tới khoá của người dùng hiện tại (HKCU).

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Registry::*;
use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};

use crate::scenes::Kind;

const PERSONALIZE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
const ACCENT: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Accent";
const DWM: &str = "Software\\Microsoft\\Windows\\DWM";

/// Các giá trị app sẽ thay đổi (khoá, tên).
const VALUES: [(&str, &str); 7] = [
    (PERSONALIZE, "ColorPrevalence"),
    (ACCENT, "AccentPalette"),
    (ACCENT, "AccentColorMenu"),
    (ACCENT, "StartColorMenu"),
    (DWM, "AccentColor"),
    (DWM, "ColorizationColor"),
    (DWM, "ColorizationAfterglow"),
];

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn backup_path() -> std::path::PathBuf {
    crate::config::dir().join("taskbar-backup.txt")
}


fn write(key: &str, name: &str, ty: u32, data: &[u8]) -> Result<(), String> {
    let (k, n) = (wide(key), wide(name));
    unsafe {
        let r = RegSetKeyValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()), ty, Some(data.as_ptr() as *const _), data.len() as u32);
        if r.is_err() {
            return Err(format!("không ghi được {name}: {r:?}"));
        }
    }
    Ok(())
}

fn delete(key: &str, name: &str) {
    let (k, n) = (wide(key), wide(name));
    unsafe {
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()));
    }
}


/// Báo cho Windows (taskbar, Start, các app) biết màu đã đổi.
fn broadcast() {
    unsafe {
        let msg = w!("ImmersiveColorSet");
        let _ = SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(msg.as_ptr() as isize), SMTO_ABORTIFHUNG, 1500, None);
        let _ = HWND::default();
    }
}


/// Taskbar đang mang màu của app (đã có bản sao lưu)?
pub fn applied() -> bool {
    backup_path().exists()
}


/// Bảng 8 màu từ sáng tới tối quanh màu chủ đề (định dạng AccentPalette của Windows).
#[allow(dead_code)]
fn palette(kind: Kind) -> [[u8; 3]; 8] {
    let (accent, dark) = match kind {
        Kind::Noel => ([0x2f, 0x9e, 0x7e], [0x0d, 0x3a, 0x34]),
        Kind::Tet => ([0xd9, 0x3a, 0x2b], [0x7a, 0x0f, 0x15]),
        Kind::Moon => ([0x6f, 0x6c, 0xd0], [0x1f, 0x22, 0x5c]),
        k => {
            let t = k.theme();
            (t.acc, [t.bar_bg.0, t.bar_bg.1, t.bar_bg.2])
        }
    };
    let mix = |a: [u8; 3], b: [u8; 3], t: f32| -> [u8; 3] {
        [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
    };
    let white = [255, 255, 255];
    let black = [0, 0, 0];
    [
        mix(accent, white, 0.6),
        mix(accent, white, 0.4),
        mix(accent, white, 0.2),
        accent,
        mix(accent, dark, 0.6),
        dark,
        mix(dark, black, 0.35),
        mix(accent, white, 0.3),
    ]
}

/// Màu hiển thị trong cửa sổ cài đặt.
#[allow(dead_code)]
pub fn preview(kind: Kind) -> [[u8; 3]; 3] {
    let p = palette(kind);
    [p[3], p[4], p[5]]
}



pub fn restore() -> Result<String, String> {
    let text = match std::fs::read_to_string(backup_path()) {
        Ok(t) => t,
        Err(_) => return Ok("Taskbar đang là màu gốc.".into()),
    };
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() != 4 || !VALUES.iter().any(|(k, n)| *k == parts[0] && *n == parts[1]) {
            continue; // chỉ khôi phục đúng những giá trị app đã đổi
        }
        let (key, name, ty, hex) = (parts[0], parts[1], parts[2], parts[3]);
        if ty == "none" {
            delete(key, name);
            continue;
        }
        let Ok(ty) = ty.parse::<u32>() else { continue };
        let data: Vec<u8> = (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()).collect();
        write(key, name, ty, &data)?;
    }
    broadcast();
    let _ = std::fs::remove_file(backup_path());
    crate::log("đã khôi phục màu taskbar gốc");
    Ok("Đã khôi phục taskbar về như cũ.".into())
}
