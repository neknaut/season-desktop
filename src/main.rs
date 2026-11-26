// Bản release không mở cửa sổ console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bar;
mod config;
mod desktop;
mod draw;
mod gfx;
mod lunar;
mod paint;
mod scenes;
mod settings;
mod taskbar;
mod tray;
mod widget;

use std::io::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tray_icon::menu::MenuEvent;
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
use windows::Win32::Foundation::HWND;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowId};

use draw::{DrawList, Tex};
use scenes::{Kind, Scene};
use settings::{Action, SettingsWin, Status};

const CHECK_EVERY: Duration = Duration::from_millis(500);

/// Ghi log ra console (bản debug) và %LOCALAPPDATA%\season-desktop\log.txt.
/// Đo thời gian (bật bằng biến môi trường SEASON_PROFILE=1): ghi vào log mỗi 5 giây.
mod prof {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    struct P {
        since: Option<Instant>,
        work: Vec<(&'static str, Duration, Duration, u32)>,
        gaps: [(Duration, Duration, u32); 2],
    }
    static P: Mutex<P> = Mutex::new(P { since: None, work: Vec::new(), gaps: [(Duration::ZERO, Duration::ZERO, 0); 2] });
    fn on() -> bool {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| std::env::var_os("SEASON_PROFILE").is_some())
    }
    pub fn add(name: &'static str, t0: Instant) {
        if !on() {
            return;
        }
        let d = t0.elapsed();
        let Ok(mut p) = P.lock() else { return };
        match p.work.iter_mut().find(|w| w.0 == name) {
            Some(w) => {
                w.1 += d;
                w.2 = w.2.max(d);
                w.3 += 1;
            }
            None => p.work.push((name, d, d, 1)),
        }
        flush(&mut p);
    }
    pub fn gap(primary: bool, d: Duration) {
        if !on() {
            return;
        }
        let Ok(mut p) = P.lock() else { return };
        let g = &mut p.gaps[primary as usize];
        g.0 += d;
        g.1 = g.1.max(d);
        g.2 += 1;
    }
    fn flush(p: &mut P) {
        let since = *p.since.get_or_insert_with(Instant::now);
        if since.elapsed() < Duration::from_secs(5) {
            return;
        }
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mut s = String::from("đo 5 giây:");
        for (name, sum, max, n) in &p.work {
            s += &format!(" | {name}: {n} lần, tb {:.2} ms, lâu nhất {:.1} ms", ms(*sum) / *n as f64, ms(*max));
        }
        for (i, (sum, max, n)) in p.gaps.iter().enumerate() {
            if *n > 0 {
                s += &format!(" | khung {}: {n} khung, cách tb {:.1} ms, lâu nhất {:.1} ms", if i == 1 { "màn chính" } else { "màn phụ" }, ms(*sum) / *n as f64, ms(*max));
            }
        }
        crate::log(&s);
        p.work.clear();
        p.gaps = [(Duration::ZERO, Duration::ZERO, 0); 2];
        p.since = Some(Instant::now());
    }
}

/// Đo thời gian một việc (xem mod prof).
pub fn prof_add(name: &'static str, t0: Instant) {
    prof::add(name, t0);
}

pub fn log(msg: &str) {
    // Chỉ in ra console ở bản debug. Bản phát hành không có console; lúc bộ cài
    // (Windows Installer) chạy app, ghi ra kênh lỗi có thể làm treo tiến trình.
    #[cfg(debug_assertions)]
    {
        let _ = writeln!(std::io::stderr(), "{msg}");
    }
    let _ = std::fs::create_dir_all(config::dir());
    let path = config::dir().join("log.txt");
    // Giữ file log nhỏ.
    if std::fs::metadata(&path).map(|m| m.len() > 512 * 1024).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "[{secs}] {msg}");
    }
}

/// Hình nền của một màn hình: cửa sổ gắn dưới icon desktop + cảnh riêng (ngang hoặc dọc).
struct Wall {
    renderer: gfx::Wallpaper,
    hwnd: HWND,
    window: Arc<Window>,
    monitor: desktop::Monitor,
    scene: Option<Box<dyn Scene>>,
    kind: Option<Kind>,
    size: (u32, u32),
    fit: (f32, [f32; 2], scenes::Layout),
    widget_key: String,
    widget_size: (f32, f32),
    widget_origin: (f32, f32),
    dl: DrawList,
    frames: u64,
    anim_time: f32,
    last_tick: Instant,
    /// Nền kính mờ cho taskbar trên màn hình này.
    backdrop: Option<tiny_skia::Pixmap>,
    /// Màn hình này đang bị che kín nên tạm nghỉ.
    covered: bool,
    /// Chủ đề đã lưu thành ảnh tĩnh làm hình nền Windows của màn này (để mở máy là thấy ngay).
    still_kind: Option<Kind>,
}

struct SettingsHardware {
    sampled: Instant,
    monitors: Vec<desktop::Monitor>,
    has_battery: bool,
}

impl SettingsHardware {
    fn read() -> Self {
        Self {
            sampled: Instant::now(),
            monitors: desktop::monitors(),
            has_battery: bar::shell::battery_percent().is_some(),
        }
    }
}

struct PreparedScene {
    kind: Kind,
    size: (u32, u32),
    built: scenes::Built,
    backdrop: Option<tiny_skia::Pixmap>,
}

impl PreparedScene {
    fn build(kind: Kind, size: (u32, u32)) -> Self {
        let built = scenes::build(kind, size.0, size.1);
        let backdrop = bar::make_backdrop(&built.back.pm, &built.front.pm, size);
        Self { kind, size, built, backdrop }
    }
}

struct ThemeJob {
    monitor: String,
    theme: String,
    cancelled: bool,
    worker: std::thread::JoinHandle<PreparedScene>,
}

struct App {
    /// Cấu hình đang áp dụng cho hình nền.
    cfg: config::Config,
    /// Bản nháp trong cửa sổ cài đặt (chỉ áp dụng khi bấm nút).
    draft: config::Config,
    message: String,
    /// Màn hình mà thông báo nói tới (rỗng = cài đặt chung): chỉ hiện ở đúng trang đó
    message_for: String,
    /// Sau khi trả ảnh gốc cho một màn hình, Explorer đưa lớp hình nền của nó lên trên (che hình
    /// nền app ở màn kia): trong khoảng này liên tục đẩy lớp đó xuống dưới cùng.
    restack_until: Option<Instant>,
    cfg_mtime: Option<SystemTime>,
    open_settings_at_start: bool,
    /// Mở app bằng tay: cửa sổ cài đặt (màn hình loading) hiện trước, hình nền và taskbar
    /// dựng ngay sau khung hình đầu tiên của nó.
    pending_start: bool,
    /// Đang lưu ảnh tĩnh ở luồng nền.
    still_job: Option<std::thread::JoinHandle<()>>,
    settings_hardware: Option<SettingsHardware>,
    theme_job: Option<ThemeJob>,
    theme_queue: std::collections::VecDeque<(String, String)>,
    gpu: Option<gfx::Gpu>,
    walls: Vec<Wall>,
    attach_error: bool,
    fonts: Option<widget::Fonts>,
    settings: Option<SettingsWin>,
    bars: Vec<bar::Bar>,
    /// Cửa sổ ẩn nhận yêu cầu thoát / gỡ cài đặt từ bộ cài
    control: Option<HWND>,
    /// Đang thoát vì gỡ cài đặt: không giữ ảnh tĩnh làm hình nền
    uninstalling: bool,
    /// Khay hệ thống riêng (nhận icon khay của các app, chuyển tiếp cho Windows)
    systray: Option<bar::systray::Host>,
    watchdog_started: bool,
    tray: Option<tray::Tray>,
    paused_user: bool,
    paused_auto: bool,
    on_battery: bool,
    autostart: bool,
    next_frame: Instant,
    /// Có cửa sổ vừa mở/đóng chờ cập nhật thanh, và lần cập nhật gần nhất
    apps_pending: bool,
    apps_refreshed: Instant,
    next_check: Instant,
    slow_tick: Instant,
}

fn same_rect(a: &windows::Win32::Foundation::RECT, b: &windows::Win32::Foundation::RECT) -> bool {
    a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
}

/// Đọc config.toml và chuyển cài đặt màn hình còn ghi theo tên cũ sang khóa theo vai trò.
fn load_config() -> config::Config {
    let mut cfg = config::load();
    let ids: Vec<(String, Vec<String>)> = desktop::monitors().into_iter().map(|m| (m.name, vec![m.hw, m.device])).collect();
    if cfg.adopt_monitor_ids(&ids) {
        log("đã chuyển cài đặt màn hình sang khóa theo vai trò (chính/phụ, ngang/dọc)");
        config::save(&cfg);
    }
    cfg
}

impl App {
    fn new(open_settings_at_start: bool) -> Self {
        let now = Instant::now();
        let cfg = load_config();
        Self {
            draft: cfg.clone(),
            message: String::new(),
            message_for: String::new(),
            restack_until: None,
            cfg,
            cfg_mtime: config::modified(),
            open_settings_at_start,
            pending_start: false,
            still_job: None,
            settings_hardware: None,
            theme_job: None,
            theme_queue: std::collections::VecDeque::new(),
            gpu: None,
            walls: Vec::new(),
            attach_error: false,
            fonts: widget::Fonts::load(),
            settings: None,
            bars: Vec::new(),
            control: None,
            uninstalling: false,
            systray: None,
            watchdog_started: false,
            tray: None,
            paused_user: false,
            paused_auto: false,
            on_battery: desktop::on_battery(),
            autostart: desktop::autostart_enabled(),
            next_frame: now,
            apps_pending: false,
            apps_refreshed: now,
            next_check: now,
            slow_tick: now,
        }
    }

    fn status(&mut self) -> Status {
        let now = desktop::local_time();
        let hardware = self.settings_hardware.get_or_insert_with(SettingsHardware::read);
        if hardware.sampled.elapsed() >= Duration::from_millis(600) {
            *hardware = SettingsHardware::read();
        }
        let mons = hardware.monitors.iter()
            .enumerate()
            .map(|(i, m)| {
                let (w, h) = m.size();
                let mc = self.cfg.mon(&m.name, m.primary);
                settings::MonInfo {
                    name: m.name.clone(),
                    title: format!("Màn hình {}", i + 1),
                    detail: format!("{w}×{h} · {} Hz", m.hz),
                    primary: m.primary,
                    portrait: m.portrait(),
                    hz: m.hz,
                    wall: self.walls.iter().find(|wl| wl.monitor.name == m.name).and_then(|wl| wl.kind),
                    bar: self.bars.iter().any(|b| b.monitor == m.name),
                    applied_theme: mc.theme.clone(),
                    applied_bar_theme: mc.taskbar_theme.clone(),
                }
            })
            .collect();
        // Cài đặt chung: so các trường không thuộc riêng màn hình nào
        let global = |c: &config::Config| {
            (c.fps, c.battery_fps, c.pause_on_fullscreen, c.pause_on_maximized, c.clock, c.keep_on_exit, c.fireworks, c.petals, c.lanterns, (c.snow * 100.0) as i32, (c.speed * 100.0) as i32)
        };
        Status {
            global_dirty: global(&self.draft) != global(&self.cfg),
            auto_kind: config::auto_scene(&now),
            next_event: config::next_event(&now),
            gpu: self.gpu.as_ref().map(|g| g.adapter.get_info().name).unwrap_or_default(),
            has_battery: hardware.has_battery,
            message: self.message.clone(),
            message_for: self.message_for.clone(),
            paused_user: self.paused_user,
            paused_auto: self.paused_auto,
            autostart: self.autostart,
            on_battery: self.on_battery,
            mons,
            loading: self.pending_start,
        }
    }

    /// Tạo hình nền cho mọi màn hình. Lỗi ở màn hình nào thì ghi log, các màn hình khác vẫn chạy.
    fn create_wallpapers(&mut self, el: &ActiveEventLoop) {
        self.walls.clear();
        for m in desktop::monitors() {
            if self.cfg.mon(&m.name, m.primary).wallpaper {
                self.create_wall(el, m);
            }
        }
        if self.walls.is_empty() {
            self.attach_error = true;
        }
    }

    fn create_wall(&mut self, el: &ActiveEventLoop, monitor: desktop::Monitor) {
        let r = monitor.rect;
        let (w, h) = monitor.size();
        let attrs = Window::default_attributes()
            .with_title("Season Desktop wallpaper")
            .with_decorations(false)
            .with_visible(false)
            .with_skip_taskbar(true)
            .with_position(PhysicalPosition::new(r.left, r.top))
            .with_inner_size(PhysicalSize::new(w, h));
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log(&format!("không tạo được cửa sổ hình nền: {e}"));
                return;
            }
        };
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => HWND(h.hwnd.get() as *mut _),
            _ => return,
        };
        match desktop::attach(hwnd, r) {
            Ok(layout) => log(&format!(
                "đã gắn hình nền màn hình {}x{} {} ({}), kiểu {layout}",
                w,
                h,
                if monitor.portrait() { "dọc" } else { "ngang" },
                if monitor.primary { "chính" } else { "phụ" }
            )),
            Err(e) => {
                log(&format!("không gắn được vào desktop: {e}"));
                return;
            }
        }
        let surface = match &self.gpu {
            Some(gpu) => match gpu.instance.create_surface(window.clone()) {
                Ok(s) => s,
                Err(e) => {
                    log(&format!("không tạo được surface: {e}"));
                    return;
                }
            },
            None => match gfx::Gpu::new(window.clone()) {
                Ok((gpu, s)) => {
                    self.gpu = Some(gpu);
                    s
                }
                Err(e) => {
                    log(&format!("không khởi tạo được GPU: {e}"));
                    return;
                }
            },
        };
        let gpu = self.gpu.as_ref().unwrap();
        let renderer = gfx::Wallpaper::new(gpu, surface, w, h);
        let now = Instant::now();
        self.walls.push(Wall {
            renderer,
            hwnd,
            window,
            monitor,
            scene: None,
            kind: None,
            size: (0, 0),
            fit: scenes::fit(w, h),
            widget_key: String::new(),
            widget_size: (0.0, 0.0),
            widget_origin: (0.0, 0.0),
            dl: DrawList::default(),
            frames: 0,
            anim_time: 0.0,
            last_tick: now,
            backdrop: None,
            covered: false,
            still_kind: None,
        });
        self.attach_error = false;
    }

    /// Dựng hình nền và taskbar của mọi màn hình theo cài đặt đã lưu.
    fn start_scenes(&mut self, el: &ActiveEventLoop) {
        if self.cfg.wallpaper_on {
            self.create_wallpapers(el);
            self.ensure_scene();
        }
        // Bật taskbar trên các màn hình đã chọn; nếu không có màn hình nào mà lần trước
        // app tắt đột ngột khi đang ẩn taskbar gốc thì trả lại ngay.
        self.sync_bars(el);
    }

    /// Khi bật "Khởi động cùng Windows": lưu cảnh đang chạy của từng màn thành ảnh tĩnh và đặt làm
    /// hình nền Windows của đúng màn đó, để lần mở máy sau Windows hiện đúng chủ đề ngay từ đầu
    /// (trước khi app kịp chạy), kể cả khi lần này máy bị tắt ngang không kịp thoát app.
    /// Chỉ làm khi chủ đề của màn đổi; mã hoá PNG và đặt hình nền chạy ở luồng nền.
    fn persist_stills(&mut self) {
        if !self.autostart || !self.cfg.wallpaper_on || self.uninstalling {
            return;
        }
        if self.still_job.as_ref().is_some_and(|j| !j.is_finished()) {
            return;
        }
        self.still_job = None;
        let Some(gpu) = &self.gpu else { return };
        let mut items: Vec<(windows::Win32::Foundation::RECT, std::path::PathBuf, tiny_skia::Pixmap)> = Vec::new();
        for wall in self.walls.iter_mut() {
            let Some(kind) = wall.kind else { continue };
            if wall.still_kind == Some(kind) || wall.frames == 0 || wall.dl.sprites.is_empty() {
                continue;
            }
            let mut dl = DrawList::default();
            dl.screen = wall.dl.screen;
            for c in wall.dl.cmds.iter().filter(|c| c.tex != Tex::Widget) {
                for s in &wall.dl.sprites[c.start as usize..c.end as usize] {
                    dl.push(c.blend, c.tex, *s);
                }
            }
            let (w, h) = wall.renderer.size();
            let px = wall.renderer.render_image(gpu, &dl, wall.fit.0, wall.fit.1);
            let Some(pm) = tiny_skia::IntSize::from_wh(w, h).and_then(|s| tiny_skia::Pixmap::from_vec(px, s)) else { continue };
            let path = config::dir().join(format!("hinh-nen-{}-{}.png", format!("{kind:?}").to_lowercase(), wall.monitor.name));
            items.push((wall.monitor.rect, path, pm));
            wall.still_kind = Some(kind);
        }
        if items.is_empty() {
            return;
        }
        self.still_job = std::thread::Builder::new().name("anh-tinh".into()).spawn(move || {
            let mut set = Vec::new();
            for (rect, path, pm) in items {
                if pm.save_png(&path).is_ok() {
                    set.push((rect, path.to_string_lossy().to_string()));
                }
            }
            if !set.is_empty() && desktop::set_still_wallpapers(&set) {
                log(&format!("đã lưu ảnh tĩnh của chủ đề làm hình nền Windows ({} màn) cho lần mở máy sau", set.len()));
            }
        }).ok();
        // Đổi hình nền Windows có thể làm Explorer đưa lớp hình nền gốc lên trên: đẩy xuống lại.
        self.restack_until = Some(Instant::now() + Duration::from_secs(5));
    }

    fn drop_walls(&mut self) {
        self.walls.clear();
    }

    fn reset_frames(&mut self) {
        for w in &mut self.walls {
            w.frames = 0;
        }
    }

    /// Dựng lại cảnh khi đổi theme / độ phân giải, và vẽ lại đồng hồ khi sang phút mới.
    fn ensure_scene(&mut self) {
        self.ensure_scene_with(None);
    }

    fn ensure_scene_with(&mut self, mut prepared: Option<(&str, PreparedScene)>) {
        let Some(gpu) = &self.gpu else { return };
        let now = desktop::local_time();
        for wall in self.walls.iter_mut() {
            let kind = config::kind_of(&self.cfg.mon(&wall.monitor.name, wall.monitor.primary).theme, &now);
            let size = wall.renderer.size();
            if Some(kind) != wall.kind || size != wall.size {
                let t0 = Instant::now();
                let prepared_for_wall = prepared.as_ref().is_some_and(|(name, scene)| {
                    *name == wall.monitor.name && scene.kind == kind && scene.size == size
                });
                let scene = if prepared_for_wall {
                    prepared.take().expect("scene matched this monitor").1
                } else {
                    PreparedScene::build(kind, size)
                };
                let PreparedScene { built: b, backdrop, .. } = scene;
                wall.renderer.set_texture(gpu, Tex::Back, b.back.pm.width(), b.back.pm.height(), b.back.pm.data());
                wall.renderer.set_texture(gpu, Tex::Front, b.front.pm.width(), b.front.pm.height(), b.front.pm.data());
                let a = &b.atlas.pm;
                wall.renderer.set_texture(gpu, Tex::Atlas, a.width(), a.height(), a.data());
                wall.backdrop = backdrop;
                for bar in self.bars.iter_mut().filter(|b| b.monitor == wall.monitor.name) {
                    bar.set_backdrop(wall.backdrop.clone());
                }
                wall.widget_origin = b.clock_position;
                wall.scene = Some(b.scene);
                wall.kind = Some(kind);
                wall.size = size;
                wall.fit = scenes::fit(size.0, size.1);
                wall.widget_key.clear();
                wall.frames = 0;
                log(&format!(
                    "dựng cảnh {} ({}) trong {} ms",
                    kind.name(),
                    if b.layout.portrait() { "dọc" } else { "ngang" },
                    t0.elapsed().as_millis()
                ));
            }
            if self.cfg.clock {
                let key = widget::key(kind, &now);
                if key != wall.widget_key {
                    if let Some(f) = &self.fonts {
                        let pm = widget::render(f, kind, &now, wall.fit.0);
                        wall.widget_size = (pm.width() as f32 / wall.fit.0, pm.height() as f32 / wall.fit.0);
                        wall.renderer.set_texture(gpu, Tex::Widget, pm.width(), pm.height(), pm.data());
                        wall.widget_key = key;
                        wall.frames = 0;
                    }
                }
            }
        }
    }

    /// Mỗi màn hình tự nghỉ khi bị che kín; màn hình khác vẫn chạy hiệu ứng.
    /// Màn hình vừa hết bị che thì chạy lại ngay (không chờ nhịp nghỉ 2 giây).
    fn update_covered(&mut self) {
        let (maxi, full) = (self.cfg.pause_on_maximized, self.cfg.pause_on_fullscreen);
        let wins = if !self.walls.is_empty() && (maxi || full) { desktop::visible_windows() } else { Vec::new() };
        let mut resumed = false;
        for w in self.walls.iter_mut() {
            let covered = desktop::covered_by(w.monitor.rect, &wins, maxi, full);
            if w.covered && !covered {
                resumed = true;
                // Bỏ qua quãng nghỉ để hiệu ứng nối tiếp mượt, không nhảy
                w.last_tick = Instant::now();
            }
            w.covered = covered;
        }
        self.paused_auto = !self.walls.is_empty() && self.walls.iter().all(|w| w.covered);
        if resumed {
            self.next_frame = Instant::now();
        }
    }

    fn draw_wall(&mut self, id: WindowId) {
        let t_draw = Instant::now();
        self.draw_wall_inner(id);
        prof::add("vẽ hình nền", t_draw);
    }

    fn draw_wall_inner(&mut self, id: WindowId) {
        let Some(gpu) = &self.gpu else { return };
        let Some(wall) = self.walls.iter_mut().find(|w| w.window.id() == id) else { return };
        let Some(scene) = &mut wall.scene else { return };
        let now = Instant::now();
        prof::gap(wall.monitor.primary, now.duration_since(wall.last_tick));
        let dt = now.duration_since(wall.last_tick).as_secs_f32().min(0.1) * self.cfg.speed;
        wall.last_tick = now;
        wall.anim_time += dt;
        let (s, off, _) = wall.fit;
        let (sw, sh) = wall.renderer.size();
        wall.dl.clear();
        wall.dl.screen = [(sw as f32 / 2.0 - off[0]) / s, (sh as f32 / 2.0 - off[1]) / s, sw as f32 / s, sh as f32 / s];
        scene.frame(wall.anim_time, dt, &self.cfg, &mut wall.dl);
        if self.cfg.clock && !wall.widget_key.is_empty() {
            let (x, y) = wall.widget_origin;
            wall.dl.image(Tex::Widget, x, y, wall.widget_size.0, wall.widget_size.1);
        }
        wall.renderer.render(gpu, &wall.dl, s, off);
        wall.frames += 1;
    }

    /// Lưu khung hình hiện tại của từng màn hình (không có đồng hồ) thành PNG
    /// và đặt làm hình nền Windows của đúng màn hình đó.
    fn save_still(&mut self) -> bool {
        let Some(gpu) = &self.gpu else { return false };
        let mut items = Vec::new();
        for (i, wall) in self.walls.iter_mut().enumerate() {
            let Some(kind) = wall.kind else { continue };
            if wall.dl.sprites.is_empty() {
                continue;
            }
            let mut dl = DrawList::default();
            dl.screen = wall.dl.screen;
            for c in wall.dl.cmds.iter().filter(|c| c.tex != Tex::Widget) {
                for s in &wall.dl.sprites[c.start as usize..c.end as usize] {
                    dl.push(c.blend, c.tex, *s);
                }
            }
            let (w, h) = wall.renderer.size();
            let px = wall.renderer.render_image(gpu, &dl, wall.fit.0, wall.fit.1);
            let Some(pm) = tiny_skia::Pixmap::from_vec(px, tiny_skia::IntSize::from_wh(w, h).unwrap()) else { continue };
            let path = config::dir().join(format!("hinh-nen-{}-man-hinh-{}.png", format!("{kind:?}").to_lowercase(), i + 1));
            if pm.save_png(&path).is_ok() {
                items.push((wall.monitor.rect, path.to_string_lossy().to_string()));
            }
        }
        !items.is_empty() && desktop::set_still_wallpapers(&items)
    }

    /// Bật/tắt taskbar riêng trên từng màn hình cho khớp với cài đặt.
    fn sync_bars(&mut self, el: &ActiveEventLoop) {
        let mons = desktop::monitors();
        // Màn hình dùng taskbar Windows: vùng làm việc đúng theo taskbar gốc.
        for m in mons.iter().filter(|m| !self.cfg.mon(&m.name, m.primary).taskbar) {
            bar::shell::release_work_area(m.rect);
        }
        let want: Vec<desktop::Monitor> = mons.into_iter().filter(|m| self.cfg.mon(&m.name, m.primary).taskbar).collect();
        self.bars.retain(|b| want.iter().any(|m| m.name == b.monitor));
        if want.is_empty() {
            bar::shell::set_covered(&[]);
            self.set_systray(false);
            if bar::shell::needs_recovery() {
                bar::shell::restore_windows_taskbar();
            }
            return;
        }
        self.set_systray(true);
        let now = desktop::local_time();
        for m in &want {
            let kind = config::kind_of(&self.cfg.mon(&m.name, m.primary).taskbar_theme, &now);
            match self.bars.iter_mut().find(|b| b.monitor == m.name) {
                Some(b) => b.set_kind(kind),
                None => self.create_bar(el, m, kind),
            }
        }
        let active_rects: Vec<_> = want.iter()
            .filter(|m| self.bars.iter().any(|b| b.monitor == m.name))
            .map(|m| m.rect)
            .collect();
        bar::shell::set_covered(&active_rects);
        if active_rects.is_empty() {
            self.set_systray(false);
            if bar::shell::needs_recovery() {
                bar::shell::restore_windows_taskbar();
            }
        } else {
            bar::shell::hide_windows_taskbar();
        }
    }

    /// Bật/tắt khay hệ thống riêng. Mỗi lần đổi thì tạo lại icon khay của chính app, để icon
    /// được gửi tới đúng khay đang chạy (khay riêng không nhận lại icon của chính tiến trình này
    /// khi phát "TaskbarCreated"): nhờ vậy bảng ^ có đúng icon hiện tại, bấm trái mở cài đặt,
    /// bấm phải hiện menu như mọi app khác.
    fn set_systray(&mut self, on: bool) {
        if on == self.systray.is_some() {
            return;
        }
        self.systray = if on { bar::systray::Host::start() } else { None };
        if self.tray.is_some() {
            self.tray = None;
            match tray::Tray::new() {
                Ok(t) => self.tray = Some(t),
                Err(e) => log(&format!("không tạo lại được icon khay: {e}")),
            }
            self.show_tray_icon();
        }
    }

    /// Icon khay chỉ hiện khi cửa sổ cài đặt không mở (app đang chạy nền), như các app thu xuống khay.
    fn show_tray_icon(&self) {
        if let Some(t) = &self.tray {
            t.set_visible(self.settings.is_none());
        }
    }

    /// Tạo taskbar riêng trên một màn hình.
    fn create_bar(&mut self, el: &ActiveEventLoop, m: &desktop::Monitor, kind: Kind) {
        let r = m.rect;
        let attrs = Window::default_attributes()
            .with_title("Season Desktop taskbar")
            .with_decorations(false)
            .with_visible(false)
            .with_skip_taskbar(true)
            .with_position(PhysicalPosition::new(r.left, r.bottom - 140))
            .with_inner_size(PhysicalSize::new((r.right - r.left) as u32, 126));
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log(&format!("không tạo được taskbar: {e}"));
                return;
            }
        };
        let backdrop = self.walls.iter().find(|w| w.monitor.name == m.name).and_then(|w| w.backdrop.clone());
        if !self.watchdog_started {
            bar::shell::spawn_watchdog();
            self.watchdog_started = true;
        }
        match bar::Bar::new(el, window, kind, backdrop, m) {
            Some(b) => self.bars.push(b),
            None => log("không bật được taskbar trên một màn hình"),
        }
    }

    fn open_settings(&mut self, el: &ActiveEventLoop) {
        if let Some(s) = &self.settings {
            s.window.set_minimized(false);
            s.window.set_visible(true);
            s.window.focus_window();
            return;
        }
        let (rgba, n) = tray::icon_rgba(64);
        let attrs = Window::default_attributes()
            .with_title("Season Desktop")
            .with_inner_size(LogicalSize::new(980.0, 720.0))
            .with_min_inner_size(LogicalSize::new(820.0, 560.0))
            .with_window_icon(winit::window::Icon::from_rgba(rgba, n, n).ok());
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log(&format!("không mở được cửa sổ cài đặt: {e}"));
                return;
            }
        };
        let mut surface = None;
        if self.gpu.is_none() {
            match gfx::Gpu::new(window.clone()) {
                Ok((g, s)) => {
                    self.gpu = Some(g);
                    surface = Some(s);
                }
                Err(e) => {
                    log(&format!("không khởi tạo được GPU cho cửa sổ cài đặt: {e}"));
                    return;
                }
            }
        }
        match SettingsWin::new(self.gpu.as_ref().unwrap(), window, surface) {
            Ok(settings) => {
                self.settings = Some(settings);
                // cửa sổ đang mở (có ô trên taskbar) thì không cần icon ở khay
                self.show_tray_icon();
            }
            Err(e) => log(&format!("không mở được cửa sổ cài đặt: {e}")),
        }
    }

    fn set_paused(&mut self, on: bool) {
        self.paused_user = on;
        if let Some(t) = &self.tray {
            t.pause.set_checked(on);
        }
        if let Some(s) = &self.settings {
            s.window.request_redraw();
        }
    }

    fn cancel_theme(&mut self, monitor: &str) {
        self.theme_queue.retain(|(name, _)| name != monitor);
        if let Some(job) = self.theme_job.as_mut().filter(|j| j.monitor == monitor) {
            job.cancelled = true;
        }
    }

    /// A scheduling change does not need new textures or an Explorer taskbar refresh
    /// if this monitor already displays the desired scene on both surfaces.
    fn reuse_active_theme(&mut self, monitor: &str, theme: &str) -> bool {
        let kind = config::kind_of(theme, &desktop::local_time());
        let Some(wall) = self.walls.iter().find(|wall| wall.monitor.name == monitor && wall.kind == Some(kind)) else {
            return false;
        };
        let primary = wall.monitor.primary;
        let active = self.cfg.mon(monitor, primary);
        if !active.wallpaper || !active.taskbar ||
            !self.bars.iter().any(|bar| bar.monitor == monitor && bar.kind() == kind) {
            return false;
        }
        self.cancel_theme(monitor);
        if active.theme != theme || active.taskbar_theme != theme {
            let applied = self.cfg.mon_mut(monitor, primary);
            applied.theme = theme.into();
            applied.taskbar_theme = theme.into();
            self.draft.monitors.clone_from(&self.cfg.monitors);
            config::save(&self.cfg);
            self.cfg_mtime = config::modified();
        }
        self.message_for = monitor.into();
        self.message = if theme == "auto" {
            "Đã bật tự động cho màn hình này.".into()
        } else {
            format!("Đã chọn chủ đề {} cho màn hình này.", kind.name())
        };
        true
    }

    fn queue_theme(&mut self, monitor: String, theme: String) {
        self.cancel_theme(&monitor);
        self.message_for = monitor.clone();
        self.message = "Đang chuẩn bị chủ đề…".into();
        self.theme_queue.push_back((monitor, theme));
    }

    fn poll_theme(&mut self, el: &ActiveEventLoop) {
        if self.theme_job.as_ref().is_some_and(|job| job.worker.is_finished()) {
            let ThemeJob { monitor, theme, cancelled, worker } = self.theme_job.take().expect("finished worker");
            if !cancelled {
                match worker.join() {
                    Ok(prepared) => self.apply_prepared_theme(monitor, theme, prepared, el),
                    Err(_) => {
                        self.message_for = monitor;
                        self.message = "Không dựng được hình nền. Xem log.txt trong thư mục cấu hình.".into();
                    }
                }
                if let Some(settings) = &self.settings {
                    settings.window.request_redraw();
                }
            }
        }
        if self.theme_job.is_some() {
            return;
        }
        while let Some((monitor, theme)) = self.theme_queue.pop_front() {
            let Some(display) = desktop::monitors().into_iter().find(|m| m.name == monitor) else { continue };
            let size = display.size();
            let kind = config::kind_of(&theme, &desktop::local_time());
            match std::thread::Builder::new().name("dung-chu-de".into()).spawn(move || PreparedScene::build(kind, size)) {
                Ok(worker) => {
                    self.theme_job = Some(ThemeJob { monitor, theme, cancelled: false, worker });
                    break;
                }
                Err(error) => {
                    log(&format!("không mở được luồng dựng chủ đề: {error}"));
                    self.message_for = monitor;
                    self.message = "Không bắt đầu được phiên dựng hình nền.".into();
                    if let Some(settings) = &self.settings {
                        settings.window.request_redraw();
                    }
                }
            }
        }
    }

    fn apply_prepared_theme(&mut self, name: String, theme: String, prepared: PreparedScene, el: &ActiveEventLoop) {
        let monitors = desktop::monitors();
        let Some((idx, m)) = monitors.iter().enumerate().find(|(_, m)| m.name == name) else { return };
        let target = config::kind_of(&theme, &desktop::local_time());
        if prepared.size != m.size() || prepared.kind != target {
            self.queue_theme(name, theme);
            return;
        }
        let previous = self.cfg.clone();
        // Capture implicit defaults before either global flag changes.
        for connected in &monitors {
            self.cfg.mon_mut(&connected.name, connected.primary);
        }
        let selected = self.cfg.mon_mut(&name, m.primary);
        selected.theme = theme.clone();
        selected.taskbar_theme = theme;
        selected.wallpaper = true;
        selected.taskbar = true;
        self.cfg.wallpaper_on = true;
        self.cfg.taskbar_on = true;
        self.attach_error = false;
        let had_wall = self.walls.iter().any(|w| w.monitor.name == name);
        if !had_wall {
            self.create_wall(el, m.clone());
        }
        self.ensure_scene_with(Some((&name, prepared)));
        let wall_ready = self.walls.iter().any(|w| w.monitor.name == name && w.kind.is_some());
        if wall_ready {
            if taskbar::applied() {
                let _ = taskbar::restore();
            }
            self.sync_bars(el);
        }
        if wall_ready && self.bars.iter().any(|b| b.monitor == name) {
            self.draft.monitors.clone_from(&self.cfg.monitors);
            self.draft.wallpaper_on = self.cfg.wallpaper_on;
            self.draft.taskbar_on = self.cfg.taskbar_on;
            config::save(&self.cfg);
            self.cfg_mtime = config::modified();
            let kind = config::kind_of(&self.cfg.mon(&name, m.primary).theme, &desktop::local_time());
            self.message_for = name.clone();
            self.message = format!("Màn hình {}: đã áp dụng chủ đề {} cho hình nền và taskbar.", idx + 1, kind.name());
        } else {
            self.cfg = previous;
            if !had_wall {
                self.walls.retain(|w| w.monitor.name != name);
            }
            self.ensure_scene();
            if wall_ready {
                self.sync_bars(el);
            }
            self.message_for = name.clone();
            self.message = format!("Màn hình {}: không áp dụng được cả bộ chủ đề. Xem log.txt trong thư mục cấu hình.", idx + 1);
        }
    }

    fn handle_actions(&mut self, actions: Vec<Action>, el: &ActiveEventLoop) {
        for a in actions {
            match a {
                Action::ApplyTheme { monitor, theme } => {
                    if !self.reuse_active_theme(&monitor, &theme) {
                        self.queue_theme(monitor, theme);
                    }
                }
                Action::ResetTheme(name) => {
                    self.cancel_theme(&name);
                    let monitors = desktop::monitors();
                    let Some((idx, m)) = monitors.iter().enumerate().find(|(_, m)| m.name == name) else { continue };
                    for connected in &monitors {
                        self.cfg.mon_mut(&connected.name, connected.primary);
                    }
                    let selected = self.cfg.mon_mut(&name, m.primary);
                    selected.wallpaper = false;
                    selected.taskbar = false;
                    self.cfg.wallpaper_on = self.cfg.monitors.iter().any(|m| m.wallpaper);
                    self.cfg.taskbar_on = self.cfg.monitors.iter().any(|m| m.taskbar);
                    self.draft.monitors.clone_from(&self.cfg.monitors);
                    self.draft.wallpaper_on = self.cfg.wallpaper_on;
                    self.draft.taskbar_on = self.cfg.taskbar_on;
                    config::save(&self.cfg);
                    self.cfg_mtime = config::modified();
                    self.walls.retain(|w| w.monitor.name != name);
                    desktop::restore_original_wallpaper_for(m.rect);
                    if !self.walls.is_empty() {
                        desktop::restack();
                        self.restack_until = Some(Instant::now() + Duration::from_secs(3));
                    }
                    self.sync_bars(el);
                    if self.bars.is_empty() && taskbar::applied() {
                        let _ = taskbar::restore();
                    }
                    self.message_for = name.clone();
                    self.message = format!("Màn hình {}: đã khôi phục hình nền và taskbar của Windows.", idx + 1);
                }
                Action::ApplyGlobal => {
                    let d = &self.draft;
                    let c = &mut self.cfg;
                    c.fps = d.fps;
                    c.battery_fps = d.battery_fps;
                    c.pause_on_fullscreen = d.pause_on_fullscreen;
                    c.pause_on_maximized = d.pause_on_maximized;
                    c.clock = d.clock;
                    c.keep_on_exit = d.keep_on_exit;
                    c.close_to_tray = d.close_to_tray;
                    c.fireworks = d.fireworks;
                    c.petals = d.petals;
                    c.lanterns = d.lanterns;
                    c.snow = d.snow;
                    c.speed = d.speed;
                    config::save(&self.cfg);
                    self.cfg_mtime = config::modified();
                    for w in self.walls.iter_mut() {
                        w.widget_key.clear();
                    }
                    self.reset_frames();
                    self.ensure_scene();
                    self.message_for.clear();
                    self.message = "Đã áp dụng cài đặt chung cho mọi màn hình.".into();
                }
                Action::TogglePause => self.set_paused(!self.paused_user),
                Action::OpenFolder => {
                    let _ = std::process::Command::new("explorer").arg(config::dir()).spawn();
                }
                Action::Autostart(on) => {
                    if let Err(e) = desktop::set_autostart(on) {
                        log(&format!("khởi động cùng Windows: {e}"));
                    }
                    self.autostart = desktop::autostart_enabled();
                }
            }
        }
    }

    fn handle_tray(&mut self, el: &ActiveEventLoop) {
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                self.open_settings(el);
            }
        }
        let mut open = false;
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            let Some(t) = &self.tray else { continue };
            if ev.id == *t.quit.id() {
                el.exit();
            } else if ev.id == *t.settings.id() {
                open = true;
            } else if ev.id == *t.pause.id() {
                let on = t.pause.is_checked();
                self.set_paused(on);
            }
        }
        if open {
            self.open_settings(el);
        }
    }

    /// Việc định kỳ: config sửa tay, nên nghỉ hay chạy, pin, Explorer khởi động lại, đổi ngày/phút.
    fn periodic(&mut self, el: &ActiveEventLoop) {
        if self.pending_start {
            // Chưa dựng lần đầu (đang hiện màn hình loading)
            return;
        }
        let m = config::modified();
        if m != self.cfg_mtime {
            let new = load_config();
            self.cfg_mtime = config::modified();
            if new != self.cfg {
                log("đã tải lại config.toml");
                self.cfg = new.clone();
                self.draft = new;
                self.reset_frames();
                if !self.cfg.wallpaper_on && !self.walls.is_empty() {
                    self.drop_walls();
                    desktop::restore_wallpaper();
                }
            }
        }
        let was = (self.paused_auto, self.on_battery);
        self.update_covered();
        let all_mons = desktop::monitors();
        self.on_battery = desktop::on_battery();
        if was != (self.paused_auto, self.on_battery) {
            if let Some(s) = &self.settings {
                s.window.request_redraw();
            }
        }
        if self.cfg.wallpaper_on {
            let mons: Vec<&desktop::Monitor> = all_mons.iter().filter(|m| self.cfg.mon(&m.name, m.primary).wallpaper).collect();
            let changed = mons.len() != self.walls.len()
                || mons.iter().any(|m| !self.walls.iter().any(|w| same_rect(&w.monitor.rect, &m.rect)));
            let lost = self.walls.iter().any(|w| !desktop::still_attached(w.hwnd));
            if !self.walls.is_empty() && (changed || lost) {
                log(if lost { "mất lớp desktop (Explorer khởi động lại?), gắn lại..." } else { "màn hình thay đổi, dựng lại hình nền" });
                self.drop_walls();
                self.create_wallpapers(el);
            } else if self.walls.is_empty() && !self.attach_error {
                self.create_wallpapers(el);
            }
        }
        self.ensure_scene();
        if let Some(h) = &self.systray {
            h.keep_on_top();
        }
        let now = desktop::local_time();
        for b in self.bars.iter_mut() {
            let primary = all_mons.iter().any(|m| m.name == b.monitor && m.primary);
            b.set_kind(config::kind_of(&self.cfg.mon(&b.monitor, primary).taskbar_theme, &now));
            let t = Instant::now();
            b.refresh();
            prof::add("  └ taskbar: làm mới", t);
        }
        let want: Vec<&String> = all_mons.iter().filter(|m| self.cfg.mon(&m.name, m.primary).taskbar).map(|m| &m.name).collect();
        if want.len() != self.bars.len() || want.iter().any(|n| !self.bars.iter().any(|b| &b.monitor == *n)) {
            self.sync_bars(el);
        }
        self.persist_stills();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.tray.is_some() || !self.walls.is_empty() {
            return;
        }
        self.control = bar::shell::create_control_window();
        match tray::Tray::new() {
            Ok(t) => self.tray = Some(t),
            Err(e) => log(&format!("không tạo được icon khay: {e}")),
        }
        if self.open_settings_at_start || self.tray.is_none() {
            // Mở bằng tay: hiện cửa sổ (màn hình loading) trước, dựng hình nền/taskbar sau
            // khung hình đầu tiên của nó (xem about_to_wait).
            self.open_settings(el);
            if self.settings.is_some() {
                self.pending_start = true;
                return;
            }
        }
        // Khởi động cùng Windows: dựng ngay theo cài đặt đã lưu, không chờ gì.
        self.start_scenes(el);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Cửa sổ cài đặt
        if self.settings.as_ref().is_some_and(|s| s.window.id() == id) {
            match event {
                WindowEvent::CloseRequested => {
                    // Nhấn X: thoát app, hoặc thu xuống khay hệ thống (giải phóng cửa sổ cho đỡ RAM;
                    // hình nền, taskbar vẫn chạy; icon hiện lại ở khay để mở lại).
                    if !self.cfg.close_to_tray || self.tray.is_none() {
                        el.exit();
                    } else {
                        self.settings = None;
                        self.show_tray_icon();
                    }
                }
                WindowEvent::RedrawRequested => {
                    let st = self.status();
                    let before = self.draft.clone();
                    let (Some(s), Some(gpu)) = (&mut self.settings, &self.gpu) else { return };
                    // Chỉ sửa bản nháp; hình nền/taskbar đổi khi bấm nút Áp dụng.
                    let t_paint = Instant::now();
                    let actions = s.paint(gpu, &mut self.draft, &st);
                    prof::add("cài đặt: vẽ", t_paint);
                    let changed = self.draft != before || !actions.is_empty();
                    // Lựa chọn nút X có hiệu lực ngay (không cần bấm Áp dụng)
                    if self.draft.close_to_tray != self.cfg.close_to_tray {
                        self.cfg.close_to_tray = self.draft.close_to_tray;
                        config::save(&self.cfg);
                        self.cfg_mtime = config::modified();
                    }
                    self.handle_actions(actions, el);
                    if changed {
                        if let Some(s) = &self.settings {
                            s.window.request_redraw();
                        }
                    }
                }
                ev => {
                    if let (Some(s), Some(gpu)) = (&mut self.settings, &self.gpu) {
                        s.on_event(gpu, &ev);
                    }
                }
            }
            return;
        }
        // Taskbar riêng
        if let Some(i) = self.bars.iter().position(|b| b.window.id() == id) {
            let action = self.bars[i].on_event(&event);
            if let Some(bar::BarAction::OpenSettings) = action {
                self.open_settings(el);
            }
            return;
        }
        // Cửa sổ hình nền của một màn hình
        match event {
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &self.gpu {
                    if let Some(w) = self.walls.iter_mut().find(|w| w.window.id() == id) {
                        w.renderer.resize(gpu, size.width, size.height);
                    }
                }
                // Không dựng lại ngay: Windows hay gửi nhiều lần đổi kích thước liên tiếp,
                // việc kiểm tra định kỳ (0,5 giây) sẽ dựng lại một lần khi kích thước đã ổn định.
            }
            WindowEvent::RedrawRequested => self.draw_wall(id),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        match bar::shell::CONTROL_REQUEST.swap(0, std::sync::atomic::Ordering::SeqCst) {
            2 => {
                // Gỡ cài đặt: dọn bằng tài khoản người dùng rồi thoát.
                log("nhận yêu cầu gỡ cài đặt: trả mọi thứ về gốc");
                self.uninstalling = true;
                if taskbar::applied() {
                    let _ = taskbar::restore();
                }
                let _ = desktop::set_autostart(false);
                el.exit();
                return;
            }
            1 => {
                log("nhận yêu cầu thoát từ bộ cài");
                el.exit();
                return;
            }
            3 => self.open_settings(el),
            _ => {}
        }
        self.handle_tray(el);
        // Màn hình loading đã hiện: giờ mới dựng hình nền và taskbar.
        if self.pending_start && self.settings.as_ref().map_or(true, |s| s.painted) {
            let t = Instant::now();
            self.start_scenes(el);
            self.pending_start = false;
            log(&format!("dựng hình nền và taskbar sau màn hình loading trong {} ms", t.elapsed().as_millis()));
            if let Some(s) = &self.settings {
                s.window.request_redraw();
            }
        }
        let now = Instant::now();
        // Cửa sổ vừa thu nhỏ/phóng to/di chuyển: xét lại ngay màn hình nào bị che
        if bar::shell::windows_changed() {
            let t = Instant::now();
            if !self.walls.is_empty() {
                self.update_covered();
            }
            if bar::shell::apps_changed() {
                self.apps_pending = true;
            }
            for b in self.bars.iter_mut() {
                b.check_fullscreen();
            }
            prof::add("xét che màn hình", t);
        }
        // App vừa mở/đóng: cập nhật thanh ngay (tối đa 10 lần mỗi giây)
        if self.apps_pending {
            if now.duration_since(self.apps_refreshed) >= Duration::from_millis(100) {
                self.apps_pending = false;
                self.apps_refreshed = now;
                for b in self.bars.iter_mut() {
                    b.refresh();
                }
            }
        }
        if now >= self.next_check {
            let t = Instant::now();
            self.periodic(el);
            prof::add("kiểm tra định kỳ", t);
            self.next_check = now + CHECK_EVERY;
        }
        self.poll_theme(el);
        let mut wake = self.next_check;
        if self.theme_job.is_some() {
            wake = wake.min(now + Duration::from_millis(24));
        }
        if let Some(s) = &mut self.settings {
            if let Some(t) = s.next_repaint {
                if now >= t {
                    s.next_repaint = None;
                    s.window.request_redraw();
                } else {
                    wake = wake.min(t);
                }
            }
        }
        let mut open_settings = false;
        for b in self.bars.iter_mut() {
            let t = Instant::now();
            b.poll_popups();
            prof::add("taskbar: cửa sổ nổi", t);
            if let Some(bar::BarAction::OpenSettings) = b.take_action() {
                open_settings = true;
            }
            if !b.is_hidden() {
                if now >= b.next_frame {
                    let t = Instant::now();
                    b.tick();
                    prof::add("taskbar: vẽ", t);
                }
                wake = wake.min(b.next_frame);
            }
        }
        if open_settings {
            self.open_settings(el);
        }
        if !self.walls.is_empty() {
            // Cảnh tĩnh (không có hiệu ứng chuyển động) coi như đang nghỉ: chỉ vẽ lại 2 giây một lần
            let moving = |w: &Wall| w.scene.as_ref().map_or(true, |s| s.animated());
            let any_running = self.walls.iter().any(|w| !(self.paused_user || w.covered || !moving(w)) || w.frames == 0);
            if now >= self.next_frame {
                let slow = self.slow_tick <= now;
                for w in self.walls.iter_mut() {
                    let paused = self.paused_user || w.covered || !moving(w);
                    if !paused || w.frames == 0 {
                        w.window.request_redraw();
                    } else {
                        // Đang nghỉ: giữ nguyên khung hình, chỉ vẽ lại 2 giây một lần để
                        // hình nền không bị đen khi Windows xoá nội dung.
                        w.last_tick = now;
                        if slow {
                            w.window.request_redraw();
                        }
                    }
                }
                if slow {
                    self.slow_tick = now + Duration::from_secs(2);
                }
                let fps = if self.on_battery { self.cfg.battery_fps } else { self.cfg.fps };
                // Giữ nhịp đều: khung sau = khung trước + chu kỳ (không cộng dồn độ trễ)
                let period = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
                self.next_frame = if any_running {
                    let n = self.next_frame + period;
                    if n <= now { now + period } else { n }
                } else {
                    self.slow_tick
                };
            }
            wake = wake.min(self.next_frame);
        }
        if self.apps_pending {
            wake = wake.min(self.apps_refreshed + Duration::from_millis(100));
        }
        if let Some(until) = self.restack_until {
            if now >= until || self.walls.is_empty() {
                self.restack_until = None;
            } else {
                desktop::restack();
                wake = wake.min(now + Duration::from_millis(30));
            }
        }
        el.set_control_flow(ControlFlow::WaitUntil(wake));
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.settings = None;
        let kept = !self.uninstalling && self.cfg.wallpaper_on && self.cfg.keep_on_exit && self.save_still();
        self.bars.clear();
        self.systray = None;
        if bar::shell::needs_recovery() {
            bar::shell::restore_windows_taskbar(); // trả lại taskbar gốc
        }
        self.drop_walls();
        self.tray = None;
        if self.uninstalling {
            desktop::restore_original_wallpaper();
            log("đã thoát để gỡ cài đặt, đã trả hình nền và taskbar gốc");
        } else if kept {
            log("đã thoát, giữ ảnh tĩnh của cảnh làm hình nền");
        } else {
            desktop::restore_wallpaper();
            log("đã thoát, trả lại hình nền gốc");
        }
    }
}

fn snapshot(dir: &str) {
    let gpu = match gfx::Gpu::headless() {
        Ok(g) => g,
        Err(e) => return eprintln!("{e}"),
    };
    let cfg = config::Config::default();
    let fonts = widget::Fonts::load();
    let now = desktop::local_time();
    for (kind, (w, h)) in Kind::ALL.into_iter().flat_map(|k| [(k, (1920u32, 1080u32)), (k, (1080, 1920))]) {
        let mut r = gfx::Wallpaper::offscreen(&gpu, w, h);
        let mut b = scenes::build(kind, w, h);
        r.set_texture(&gpu, Tex::Back, b.back.pm.width(), b.back.pm.height(), b.back.pm.data());
        r.set_texture(&gpu, Tex::Front, b.front.pm.width(), b.front.pm.height(), b.front.pm.data());
        r.set_texture(&gpu, Tex::Atlas, b.atlas.pm.width(), b.atlas.pm.height(), b.atlas.pm.data());
        let (s, off, l) = scenes::fit(w, h);
        let widget_size = fonts.as_ref().map(|f| {
            let pm = widget::render(f, kind, &now, s);
            r.set_texture(&gpu, Tex::Widget, pm.width(), pm.height(), pm.data());
            (pm.width() as f32 / s, pm.height() as f32 / s)
        });
        let mut dl = DrawList::default();
        dl.screen = [(w as f32 / 2.0 - off[0]) / s, (h as f32 / 2.0 - off[1]) / s, w as f32 / s, h as f32 / s];
        // chạy 4 giây để hạt, pháo hoa, khói kịp xuất hiện
        for i in 0..120 {
            dl.clear();
            b.scene.frame(i as f32 / 30.0, 1.0 / 30.0, &cfg, &mut dl);
        }
        if let Some((width, height)) = widget_size {
            let (x, y) = b.clock_position;
            dl.image(Tex::Widget, x, y, width, height);
        }
        let px = r.render_image(&gpu, &dl, s, off);
        let path = format!("{dir}/{kind:?}-{}.png", if l.portrait() { "doc" } else { "ngang" }).to_lowercase();
        if let Some(pm) = tiny_skia::Pixmap::from_vec(px, tiny_skia::IntSize::from_wh(w, h).unwrap()) {
            let _ = pm.save_png(&path);
            println!("đã lưu {path}");
        }
    }
}

fn main() {
    // Đồng hồ hẹn giờ của Windows mặc định chỉ chính xác ~15,6 ms làm khung hình lúc nhanh lúc chậm
    // (giật). Xin độ chính xác 1 ms để hình nền chạy đều.
    unsafe {
        let _ = windows::Win32::Media::timeBeginPeriod(1);
    }
    bar::shell::watch_foreground();
    // Nếu app gặp lỗi nghiêm trọng, vẫn trả taskbar gốc cho Windows trước khi tắt.
    std::panic::set_hook(Box::new(|info| {
        log(&format!("app gặp lỗi: {info}"));
        if bar::shell::needs_recovery() {
            bar::shell::restore_windows_taskbar();
        }
    }));
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--watchdog") {
        if let Some(pid) = args.get(i + 1).and_then(|p| p.parse().ok()) {
            bar::shell::watchdog(pid);
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--export-icon") {
        // Xuất icon của app ra file .ico (dùng khi build bộ cài).
        let path = args.get(i + 1).map(String::as_str).unwrap_or("icon.ico");
        match std::fs::write(path, tray::icon_ico()) {
            Ok(()) => println!("đã lưu {path}"),
            Err(e) => eprintln!("không lưu được {path}: {e}"),
        }
        return;
    }
    if args.iter().any(|a| a == "--quit-app") {
        // Bộ cài gọi trước khi gỡ/nâng cấp: nhắn app tự thoát (app tự trả taskbar gốc);
        // không kịp thì tắt hẳn (tiến trình canh sẽ trả taskbar gốc).
        if bar::shell::send_control(false) == 0 || !bar::shell::wait_instances_exit(6) {
            bar::shell::close_instances(true);
        }
        return;
    }
    if args.iter().any(|a| a == "--uninstall-app") {
        // Bộ cài gọi khi gỡ hẳn: nhắn app đang chạy tự dọn (bằng tài khoản người dùng) rồi thoát.
        if bar::shell::send_control(true) > 0 {
            bar::shell::wait_instances_exit(8);
        }
        return;
    }
    let restore_only = args.iter().any(|a| a == "--restore-only");
    if restore_only || args.iter().any(|a| a == "--restore") {
        // Trả mọi thứ về như trước khi dùng app. --restore tắt app trước; --restore-only
        // (bộ cài gọi bằng tài khoản người dùng) thì không.
        log(&format!("khôi phục: bắt đầu ({})", if restore_only { "--restore-only" } else { "--restore" }));
        if !restore_only {
            bar::shell::close_instances(true);
        }
        if bar::shell::needs_recovery() {
            bar::shell::restore_windows_taskbar();
        }
        if taskbar::applied() {
            let _ = taskbar::restore();
        }
        log("khôi phục: hình nền");
        desktop::restore_original_wallpaper();
        log("khôi phục: khởi động cùng Windows");
        let _ = desktop::set_autostart(false);
        log(&format!("đã khôi phục toàn bộ ({})", if restore_only { "--restore-only" } else { "--restore" }));
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        return snapshot(args.get(i + 1).map(String::as_str).unwrap_or("."));
    }
    if !desktop::single_instance() {
        // Đã có một bản đang chạy: không mở thêm, nhắn bản đó mở cửa sổ cài đặt
        // (như bấm vào icon app trên taskbar / Start khi app đang chạy).
        if std::env::args().any(|a| a == "--minimized") {
            log("Season Desktop đang chạy rồi.");
        } else {
            let ok = bar::shell::send_open();
            log(if ok { "Season Desktop đang chạy: đã mở cửa sổ cài đặt" } else { "Season Desktop đang chạy rồi (không nhắn được)." });
        }
        return;
    }
    let minimized = std::env::args().any(|a| a == "--minimized");
    log("khởi động Season Desktop");
    let el = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            log(&format!("không tạo được event loop: {e}"));
            return;
        }
    };
    let mut app = App::new(!minimized);
    if let Err(e) = el.run_app(&mut app) {
        log(&format!("event loop lỗi: {e}"));
    }
}
