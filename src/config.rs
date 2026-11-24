//! Cấu hình người dùng: %APPDATA%\season-desktop\config.toml.
//! Cửa sổ cài đặt ghi vào đây; sửa tay file này app cũng tự áp dụng.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::desktop::LocalTime;
use crate::scenes::Kind;
use crate::widget::{days, tet_dates};

/// Cài đặt riêng của một màn hình (nhận diện theo tên thiết bị, ví dụ \.\DISPLAY1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MonitorCfg {
    pub name: String,
    /// "auto", "noel", "tet", "moon"
    pub theme: String,
    /// Hình nền động trên màn hình này
    pub wallpaper: bool,
    /// Taskbar Season Desktop trên màn hình này
    pub taskbar: bool,
    /// Chủ đề của taskbar (lưu lúc bấm Áp dụng taskbar)
    pub taskbar_theme: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// "auto" (theo ngày trong năm), "noel", "tet" hoặc "moon".
    pub theme: String,
    /// false = đã bấm "Khôi phục hình nền gốc", app không vẽ hình nền.
    pub wallpaper_on: bool,
    /// Khi thoát app: đặt ảnh tĩnh của cảnh hiện tại làm hình nền Windows.
    pub keep_on_exit: bool,
    /// Nhấn X ở cửa sổ cài đặt: true = thu xuống khay hệ thống (app vẫn chạy), false = thoát app.
    pub close_to_tray: bool,
    /// Đang dùng taskbar riêng của app.
    pub taskbar_on: bool,
    /// Chủ đề của taskbar: "auto", "noel", "tet", "moon".
    pub taskbar_theme: String,
    pub fps: u32,
    pub battery_fps: u32,
    pub pause_on_fullscreen: bool,
    pub pause_on_maximized: bool,
    pub speed: f32,
    pub clock: bool,
    pub snow: f32,
    pub fireworks: bool,
    pub petals: bool,
    pub lanterns: bool,
    /// Cài đặt từng màn hình (để cuối cho đúng định dạng TOML).
    pub monitors: Vec<MonitorCfg>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "auto".into(),
            wallpaper_on: true,
            keep_on_exit: true,
            close_to_tray: true,
            taskbar_on: false,
            taskbar_theme: "auto".into(),
            fps: 30,
            battery_fps: 15,
            pause_on_fullscreen: true,
            pause_on_maximized: true,
            speed: 1.0,
            clock: true,
            snow: 1.0,
            fireworks: true,
            petals: true,
            lanterns: true,
            monitors: Vec::new(),
        }
    }
}

impl Config {
    fn sanitize(mut self) -> Self {
        if !["auto", "noel", "tet", "moon"].contains(&self.theme.as_str()) {
            self.theme = "auto".into();
        }
        self.fps = self.fps.clamp(5, 240);
        self.battery_fps = self.battery_fps.clamp(5, 240);
        self.snow = self.snow.clamp(0.0, 2.0);
        self.speed = self.speed.clamp(0.25, 2.0);
        self
    }

    /// Cài đặt của một màn hình; màn hình mới thấy lần đầu thì lấy theo cài đặt chung.
    pub fn mon(&self, name: &str, primary: bool) -> MonitorCfg {
        self.monitors.iter().find(|m| m.name == name).cloned().unwrap_or_else(|| MonitorCfg {
            name: name.to_string(),
            theme: self.theme.clone(),
            wallpaper: true,
            // taskbar riêng đang bật thì màn hình mới cũng có taskbar
            taskbar: self.taskbar_on || (primary && self.monitors.iter().any(|m| m.taskbar)),
            taskbar_theme: self.taskbar_theme.clone(),
        })
    }

    pub fn mon_mut(&mut self, name: &str, primary: bool) -> &mut MonitorCfg {
        if !self.monitors.iter().any(|m| m.name == name) {
            let m = self.mon(name, primary);
            self.monitors.push(m);
        }
        self.monitors.iter_mut().find(|m| m.name == name).unwrap()
    }
}

pub fn kind_of(theme: &str, now: &LocalTime) -> Kind {
    {
        match Kind::from_id(theme) {
            Some(k) => k,
            None => auto_scene(now),
        }
    }
}

/// Rằm tháng Tám (Tết Trung thu) theo dương lịch.
const MID_AUTUMN: [(i32, u32, u32); 5] = [(2025, 10, 6), (2026, 9, 25), (2027, 9, 15), (2028, 10, 3), (2029, 9, 22)];
const TET_EXTRA: [(i32, u32, u32); 1] = [(2026, 2, 17)];

/// Hình nền ngày lễ bắt đầu từ bao nhiêu ngày trước ngày lễ.
pub const LEAD_DAYS: i64 = 3;

/// Các ngày lễ: (ngày bắt đầu hiện, ngày kết thúc, chủ đề, ngày chính) – tính bằng số ngày.
/// Mỗi dịp hiện từ 3 ngày trước tới hết ngày lễ; riêng Giáng sinh và Tết dương lịch từ 1 tuần
/// trước, Tết Nguyên Đán từ 1 tháng trước (tới hết mùng 5).
pub fn events(year: i32) -> Vec<(i64, i64, Kind, i64)> {
    let mut v = Vec::new();
    let mut add = |main: i64, extra: i64, k: Kind| {
        let lead = match k {
            Kind::Noel | Kind::NewYear => 7,
            Kind::Tet => 30,
            _ => LEAD_DAYS,
        };
        v.push((main - lead, main + extra, k, main))
    };
    for y in year - 1..=year + 1 {
        add(days(y, 1, 1), 0, Kind::NewYear);
        add(days(y, 2, 14), 0, Kind::Valentine);
        add(days(y, 3, 8), 0, Kind::Women);
        add(days(y, 4, 30), 1, Kind::Reunify); // 30/4 – 1/5
        add(days(y, 6, 1), 0, Kind::Children);
        add(days(y, 9, 2), 0, Kind::National);
        add(days(y, 10, 20), 0, Kind::VnWomen);
        add(days(y, 10, 31), 0, Kind::Halloween);
        add(days(y, 11, 20), 0, Kind::Teachers);
        add(days(y, 12, 25), 1, Kind::Noel); // 22/12 – 26/12
    }
    let tets = tet_dates().iter().map(|&(y, m, d, _)| (y, m, d)).chain(TET_EXTRA);
    for (y, m, d) in tets {
        add(days(y, m, d), 4, Kind::Tet); // tới hết mùng 5
    }
    for (y, m, d) in MID_AUTUMN {
        add(days(y, m, d), 0, Kind::Moon);
    }
    v
}

/// Mùa theo tháng (miền Bắc): xuân 2–4, hạ 5–7, thu 8–10, đông 11–1.
pub fn season(month: u32) -> Kind {
    match month {
        2..=4 => Kind::Spring,
        5..=7 => Kind::Summer,
        8..=10 => Kind::Autumn,
        _ => Kind::Winter,
    }
}

/// Tự chọn chủ đề: trong dịp lễ (3 ngày trước tới hết ngày lễ) thì dùng ngày lễ đó;
/// ngày thường thì theo mùa.
pub fn auto_scene(now: &LocalTime) -> Kind {
    let today = days(now.year, now.month, now.day);
    let ev = events(now.year);
    if let Some(e) = ev.iter().filter(|e| today >= e.0 && today <= e.1).min_by_key(|e| (e.3 - today).abs()) {
        return e.2;
    }
    season(now.month)
}

/// Sự kiện chính sắp tới (hoặc hôm nay) và còn bao nhiêu ngày.
pub fn next_event(now: &LocalTime) -> Option<(Kind, i64)> {
    let today = days(now.year, now.month, now.day);
    events(now.year).into_iter().filter(|e| e.3 >= today).min_by_key(|e| e.3).map(|e| (e.2, e.3 - today))
}

/// Thư mục AppData\Roaming của NGƯỜI DÙNG (hỏi thẳng Windows, không dựa vào biến môi trường,
/// vì lúc gỡ cài đặt app được Windows Installer chạy với môi trường khác).
fn roaming_appdata() -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_RoamingAppData, SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s.map(PathBuf::from)
    }
}

pub fn dir() -> PathBuf {
    let base = roaming_appdata()
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("season-desktop")
}

pub fn path() -> PathBuf {
    dir().join("config.toml")
}

pub fn modified() -> Option<SystemTime> {
    std::fs::metadata(path()).and_then(|m| m.modified()).ok()
}

pub fn load() -> Config {
    match std::fs::read_to_string(path()) {
        Ok(text) => match toml::from_str::<Config>(&text) {
            Ok(c) => c.sanitize(),
            Err(e) => {
                crate::log(&format!("config.toml lỗi, dùng mặc định: {e}"));
                Config::default()
            }
        },
        Err(_) => {
            let c = Config::default();
            save(&c);
            c
        }
    }
}

/// Ghi config. Ghi ra file tạm rồi đổi tên, để file không bao giờ bị hỏng giữa chừng.
pub fn save(cfg: &Config) {
    let _ = std::fs::create_dir_all(dir());
    let body = toml::to_string_pretty(cfg).unwrap_or_default();
    let text = format!("# Season Desktop - cấu hình (cửa sổ cài đặt tự ghi file này)\n\n{body}");
    let tmp = dir().join("config.toml.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, path());
    }
}
