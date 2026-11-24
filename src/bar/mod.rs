//! Taskbar riêng kiểu "ba hòn đảo" kính mờ như bản xem thử:
//! trái = nhạc đang phát / lời chúc, giữa = Start + các app đang mở, phải = hệ thống + đồng hồ.
//! Trang trí theo mùa: dây đèn + tuyết (Giáng sinh), nhành mai + lì xì (Tết), đèn ông sao (Trung thu).
//!
//! Vẽ bằng CPU (tiny-skia) lên cửa sổ trong suốt từng điểm ảnh; chỗ trong suốt thì chuột
//! bấm xuyên qua. Taskbar gốc của Windows chỉ bị ẩn, luôn được trả lại khi tắt/thoát.

pub mod jumplist;
pub mod media;
pub mod menu;
pub mod preview;
pub mod shell;
pub mod systray;

use std::sync::Arc;
use std::time::{Duration, Instant};

use ab_glyph::FontVec;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tiny_skia::*;
use windows::Win32::Foundation::{HWND, RECT};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::desktop::{local_time, Monitor};
use crate::paint::{c, ca, rad, round_rect_path, solid, star_path, Canvas};
use crate::scenes::Kind;
use crate::widget;

/// Kích thước (đơn vị logic, nhân với DPI): thanh cao 58, cách đáy 12, chừa 56 phía trên cho trang trí.
const BAR_H: f32 = 58.0;
const TOP: f32 = 56.0;
const BOTTOM: f32 = 5.0;
/// Thu nhỏ cả thanh để chiều cao chiếm chỗ gần bằng taskbar gốc của Windows (48 px ở 100%).
const SCALE: f32 = 0.78;
const SIDE: f32 = 16.0;
pub const WIN_H: f32 = TOP + BAR_H + BOTTOM;
/// Phần giữ chỗ ở đáy màn hình để cửa sổ phóng to không đè lên thanh.
const RESERVE: f32 = BAR_H + BOTTOM + 2.0;
/// Chiều cao (pixel) dải hình nền dùng làm nền kính mờ.
pub const BAND: u32 = 340;
const FPS: u64 = 12;

/// Vị trí các phần chuyển động (vẽ lại mỗi khung hình); phần còn lại được vẽ sẵn vào cache.
#[derive(Default, Clone, Copy)]
struct Anim {
    eq: Option<(f32, f32, [u8; 3])>,
    lights: Option<(f32, f32, f32)>,
    lantern: Option<(f32, f32)>,
}

impl Anim {
    fn any(&self) -> bool {
        self.eq.is_some() || self.lights.is_some() || self.lantern.is_some()
    }
}

struct Look {
    bg: (u8, u8, u8, f32),
    border: (u8, u8, u8, f32),
    fg: [u8; 3],
    acc: [u8; 3],
}

fn look(kind: Kind) -> Look {
    let theme = kind.theme();
    Look { bg: theme.bar_bg, border: theme.bar_border, fg: theme.fg, acc: theme.acc }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hit {
    Start,
    App(isize),
    Media,
    Settings,
    Peek,
    /// Mạng, pin: mở cài đặt nhanh của Windows
    Net,
    /// Loa: bấm mở cài đặt nhanh, cuộn chuột để chỉnh âm lượng
    Volume,
    Clock,
    /// Icon ghim ở khay hệ thống (thứ tự trong khay)
    Tray(usize),
}

enum FlyEv {
    Move(f32, f32),
    Leave,
    Up(bool),
}

pub enum BarAction {
    OpenSettings,
}

/// Menu đang mở là của cái gì (để biết lựa chọn áp vào đâu).
enum MenuCtx {
    None,
    System,
    App { hwnd: isize, lnk: Option<String>, exe: String, wins: Vec<isize> },
}

/// ID app của một ô, cùng dạng với "Appid" trên taskbar gốc (chữ thường): AUMID, không thì đường dẫn exe.
fn tile_appid(t: &Tile, pins: &[shell::Pin]) -> String {
    if let Some(p) = t.lnk.as_ref().and_then(|l| pins.iter().find(|p| &p.lnk == l)) {
        return if p.aumid.is_empty() { p.target.clone() } else { p.aumid.to_lowercase() };
    }
    t.alias.strip_prefix("id:").or_else(|| t.alias.strip_prefix("exe:")).unwrap_or(&t.alias).to_lowercase()
}

struct Drag {
    key: String,
    /// x lúc nhấn chuột (chưa rê đủ xa thì vẫn là bấm)
    press_x: f32,
    /// khoảng cách từ mép trái ô tới con trỏ, để ô không giật về giữa con trỏ
    grab: f32,
    active: bool,
}

struct Tile {
    /// Cửa sổ mới dùng gần nhất của app; số âm = app đã ghim nhưng chưa mở
    hwnd: isize,
    /// Mọi cửa sổ của app (gộp chung một ô như taskbar gốc), mới dùng trước
    wins: Vec<isize>,
    /// Khoá nhận diện ô (giữ icon và thứ tự ổn định)
    key: String,
    /// Khoá theo app đang chạy (ID app / chương trình): giữ nguyên chỗ khi ghim hoặc bỏ ghim
    alias: String,
    title: String,
    icon: Option<Pixmap>,
    /// File .lnk nếu là app đã ghim
    lnk: Option<String>,
}

/// Lịch tháng (bấm vào ngày giờ), vẽ theo chủ đề.
struct Calendar {
    hwnd: HWND,
    layered: Option<shell::Layered>,
    open: bool,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    /// Lệch so với tháng hiện tại (bấm ‹ ›)
    month_off: i32,
    hover: Option<i32>,
    close_at: Option<Instant>,
    fade: Option<shell::Fade>,
}

const CAL_W: f32 = 320.0;
const CAL_H: f32 = 372.0;

/// Bảng icon ẩn (mở bằng nút ^), vẽ theo chủ đề của taskbar.
struct Flyout {
    hwnd: HWND,
    layered: Option<shell::Layered>,
    open: bool,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    cols: usize,
    hover: Option<usize>,
    hits: Vec<(Rect, usize)>,
    close_at: Option<Instant>,
    /// Vẽ ở nhịp sau (không vẽ ngay trong lúc đang xử lý cú bấm)
    pending: bool,
    fade: Option<shell::Fade>,
}

/// Khoảng trống phía trên bảng ^ để đặt trang trí (tuyết...).
const FLY_TOP: f32 = 10.0;
/// Thời gian hiệu ứng ô app (ms): trượt tới chỗ mới, lướt lên khi mở / lướt xuống khi đóng.
const TILE_SLIDE_MS: f32 = 220.0;
const TILE_APPEAR_MS: f32 = 240.0;
const FLY_CELL: f32 = 44.0;

struct Fonts {
    regular: FontVec,
    semibold: FontVec,
}

pub struct Bar {
    pub window: Arc<Window>,
    /// Màn hình chính: đủ ba đảo; màn hình phụ chỉ có đảo giữa (Start + app)
    primary: bool,
    /// Tên màn hình mà thanh này nằm trên.
    pub monitor: String,
    hwnd: HWND,
    kind: Kind,
    k: f32,
    mon: RECT,
    wpx: i32,
    hpx: i32,
    layered: Option<shell::Layered>,
    fonts: Option<Fonts>,
    backdrop: Option<Pixmap>,
    apps: Vec<Tile>,
    fg: isize,
    media: media::Media,
    minfo: media::MediaInfo,
    hover: Option<Hit>,
    cursor: (f32, f32),
    hits: Vec<(Rect, Hit)>,
    hidden: bool,
    t0: Instant,
    pub next_frame: Instant,
    battery: Option<u8>,
    /// Icon ghim ở khay hệ thống: (tên hiển thị, ảnh icon thật)
    /// Icon khay ghim ra ngoài (hiện trên đảo phải) và icon ẩn (trong bảng ^).
    tray_pinned: Vec<(systray::TrayIcon, Pixmap)>,
    tray_hidden: Vec<(systray::TrayIcon, Pixmap)>,
    tray_gen: u64,
    promoted: std::collections::HashSet<String>,
    tray_check: Instant,
    /// App đã ghim trên taskbar Windows (đọc lại mỗi 10 giây)
    pins: Vec<shell::Pin>,
    pins_check: Instant,
    /// Dấu vân tay của danh sách ghim lần đọc trước, và kết quả đang đọc ở luồng riêng
    pins_sig: Vec<u8>,
    pins_rx: Option<std::sync::mpsc::Receiver<Vec<shell::Pin>>>,
    /// Số lần đã đăng ký lại chỗ giữ ở đáy màn hình
    appbar_tries: u32,
    /// Đường dẫn chương trình của từng cửa sổ (đỡ phải hỏi lại mỗi lần)
    exe_cache: std::collections::HashMap<isize, String>,
    /// AppUserModelID của từng cửa sổ (Windows dùng để gộp cửa sổ, ví dụ mỗi hồ sơ Chrome một ô)
    aumid_cache: std::collections::HashMap<isize, String>,
    /// Âm lượng loa mặc định
    audio: Option<shell::Audio>,
    vol: (f32, bool),
    audio_check: Instant,
    cal: Calendar,
    /// Lúc cửa sổ nổi vừa bị đóng do bấm ra ngoài (để cú bấm vào chính nút đó không mở lại ngay)
    outside_closed: Option<(Instant, u8)>,
    /// Ảnh xem trước cửa sổ khi rê chuột vào app
    preview: preview::Preview,
    /// Hẹn mở xem trước (sau một nhịp rê chuột) cho ô app nào
    preview_at: Option<(Instant, isize)>,
    /// Hẹn đóng xem trước khi chuột rời đi
    preview_hide_at: Option<Instant>,
    /// Vị trí (x logic) của từng ô app trên thanh
    tile_pos: Vec<(f32, isize)>,
    /// Hiệu ứng ô app như taskbar gốc: trượt tới chỗ mới (từ, tới, lúc bắt đầu)
    tile_slide: std::collections::HashMap<String, (f32, f32, Instant)>,
    /// Ô vừa xuất hiện (app mới mở): lướt từ dưới lên
    tile_born: std::collections::HashMap<String, Instant>,
    /// Ô vừa biến mất (app vừa đóng): lướt xuống rồi mất (khoá, x, icon, lúc bắt đầu)
    tile_dying: Vec<(String, f32, Option<Pixmap>, Instant)>,
    /// Icon cuối cùng của từng ô (để vẽ lúc ô đang biến mất)
    tile_icons: std::collections::HashMap<String, Pixmap>,
    /// Bề rộng đảo giữa đang co giãn (từ, tới, lúc bắt đầu)
    island_w: (f32, f32, Instant),
    /// Góc xoay mũi tên ^ (từ, tới, lúc bắt đầu): xoay nửa vòng khi mở/đóng bảng icon ẩn như taskbar gốc
    chev: (f32, f32, Instant),
    /// Kéo ô app sang trái/phải để đổi chỗ (như taskbar gốc)
    drag: Option<Drag>,
    /// Vị trí x (logic) của ô đầu tiên trên đảo giữa, để tính chỗ thả khi kéo
    tiles_x0: f32,
    /// Thời điểm mở tiến trình của từng cửa sổ (xếp app đang mở theo thứ tự mở)
    start_cache: std::collections::HashMap<isize, u64>,
    /// Lần đổi thứ tự (từ màn hình khác) đã áp dụng
    order_gen: u64,
    /// Lần đổi thứ tự của taskbar gốc đã áp dụng
    native_gen: u64,
    /// Còn hiệu ứng ô đang chạy (vẽ lại liên tục cho tới khi xong)
    tiles_animating: bool,
    /// Menu chuột phải kiểu Windows 11
    menu: menu::Menu,
    menu_ctx: MenuCtx,
    /// Các mục jump list của menu đang mở
    jump: Vec<jumplist::Entry>,
    /// Việc cần app chính làm (từ menu)
    action: Option<BarAction>,
    /// Bộ đệm dùng lại mỗi khung hình (đỡ cấp phát bộ nhớ)
    work: Option<Pixmap>,
    /// Tâm các icon khay và nút ^ trên đảo phải (đơn vị logic)
    tray_pos: Vec<f32>,
    chevron_x: f32,
    clock_x: f32,
    flyout: Flyout,
    peek_until: Option<Instant>,
    net: shell::Net,
    net_check: Instant,
    tray: Option<isize>,
    closed: bool,
    /// Ảnh tĩnh đã vẽ sẵn (đảo kính, icon, chữ); chỉ vẽ lại khi có thay đổi.
    cache: Option<Pixmap>,
    dirty: bool,
    anim: Anim,
    sig: String,
}

fn hwnd_of(w: &Window) -> Option<HWND> {
    match w.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    }
}

/// Tạo nền kính mờ từ dải dưới cùng của hình nền (lớp nền + tiền cảnh).
pub fn make_backdrop(back: &Pixmap, front: &Pixmap, screen: (u32, u32)) -> Option<Pixmap> {
    let (sw, sh) = screen;
    let band = BAND.min(sh);
    let (w, h) = ((sw / 8).max(1), (band / 8).max(1));
    let mut small = Pixmap::new(w, h)?;
    let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
    let band_scale = h as f32 / band as f32;
    let offset = -((sh - band) as f32 * band_scale);
    for image in [back, front] {
        let transform = Transform::from_row(
            w as f32 / image.width() as f32, 0.0,
            0.0, band_scale * sh as f32 / image.height() as f32,
            0.0, offset,
        );
        small.draw_pixmap(0, 0, image.as_ref(), &paint, transform, None);
    }
    let mut scratch = vec![0; small.data().len()];
    for _ in 0..3 {
        box_blur(&mut small, &mut scratch, 2);
    }
    Some(small)
}

fn box_blur(pm: &mut Pixmap, tmp: &mut [u8], r: i32) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let src = pm.data();
    let idx = |x: i32, y: i32| ((y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) * 4) as usize;
    let n = (2 * r + 1) as u32;
    for y in 0..h {
        for x in 0..w {
            for ch in 0..4 {
                let s: u32 = (-r..=r).map(|d| src[idx(x + d, y) + ch] as u32).sum();
                tmp[idx(x, y) + ch] = (s / n) as u8;
            }
        }
    }
    let out = pm.data_mut();
    for y in 0..h {
        for x in 0..w {
            for ch in 0..4 {
                let s: u32 = (-r..=r).map(|d| tmp[idx(x, y + d) + ch] as u32).sum();
                out[idx(x, y) + ch] = (s / n) as u8;
            }
        }
    }
}

fn truncate(font: &FontVec, px: f32, s: &str, max: f32) -> String {
    if widget::measure(font, px, s) <= max {
        return s.to_string();
    }
    let mut out: String = s.to_string();
    while !out.is_empty() && widget::measure(font, px, &(out.clone() + "…")) > max {
        out.pop();
    }
    out.trim_end().to_string() + "…"
}

/// Đường cong (cung tròn) dạng nhiều đoạn thẳng.
fn arc(pb: &mut PathBuilder, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, first: bool) {
    let n = 16;
    for i in 0..=n {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        let (x, y) = (cx + a.cos() * r, cy + a.sin() * r);
        if i == 0 && first {
            pb.move_to(x, y)
        } else {
            pb.line_to(x, y)
        }
    }
}

#[derive(Clone, Copy)]
enum Icon {
    Snowflake,
    Chevron,
    ChevronDown,
    Wifi,
    Ethernet,
    /// (mức 0..1, tắt tiếng)
    Volume(f32, bool),
    Offline,
    Battery(u8),
}

/// Vẽ icon nét mảnh trong ô 24x24 (giống bộ icon của bản xem thử), đặt tại (x, y) cỡ `s`.
fn icon(cv: &mut Canvas, which: Icon, x: f32, y: f32, s: f32, col: Color) {
    let t = Transform::from_translate(x, y).pre_scale(s / 24.0, s / 24.0);
    let sw = 1.8 * s / 24.0;
    let mut paths: Vec<Path> = Vec::new();
    let mut fills: Vec<Path> = Vec::new();
    match which {
        Icon::Snowflake => {
            for i in 0..3 {
                let a = i as f32 * std::f32::consts::PI / 3.0 + std::f32::consts::FRAC_PI_2;
                let mut pb = PathBuilder::new();
                pb.move_to(12.0 + a.cos() * 9.0, 12.0 + a.sin() * 9.0);
                pb.line_to(12.0 - a.cos() * 9.0, 12.0 - a.sin() * 9.0);
                paths.extend(pb.finish());
            }
        }
        Icon::Chevron => {
            let mut pb = PathBuilder::new();
            pb.move_to(7.0, 14.5);
            pb.line_to(12.0, 9.5);
            pb.line_to(17.0, 14.5);
            paths.extend(pb.finish());
        }
        Icon::ChevronDown => {
            let mut pb = PathBuilder::new();
            pb.move_to(7.0, 9.5);
            pb.line_to(12.0, 14.5);
            pb.line_to(17.0, 9.5);
            paths.extend(pb.finish());
        }
        Icon::Wifi => {
            for (r, half) in [(14.0f32, 9.5f32), (9.5, 6.2), (4.6, 2.8)] {
                let a = (half / r).acos();
                let mut pb = PathBuilder::new();
                arc(&mut pb, 12.0, 19.5, r, -(std::f32::consts::PI - a), -a, true);
                paths.extend(pb.finish());
            }
            fills.extend(PathBuilder::from_circle(12.0, 19.2, 1.1));
        }
        Icon::Ethernet => {
            // màn hình có dây mạng
            paths.extend(round_rect_path(3.5, 4.0, 17.0, 11.5, 2.0));
            for (x0, y0, x1, y1) in [(12.0, 15.5, 12.0, 19.5), (7.5, 19.5, 16.5, 19.5)] {
                let mut pb = PathBuilder::new();
                pb.move_to(x0, y0);
                pb.line_to(x1, y1);
                paths.extend(pb.finish());
            }
        }
        Icon::Offline => {
            paths.extend(PathBuilder::from_circle(12.0, 12.0, 8.0));
            let mut pb = PathBuilder::new();
            pb.move_to(5.5, 18.5);
            pb.line_to(18.5, 5.5);
            paths.extend(pb.finish());
        }
        Icon::Volume(level, muted) => {
            let mut pb = PathBuilder::new();
            pb.move_to(4.0, 10.0);
            pb.line_to(4.0, 14.0);
            pb.line_to(8.0, 14.0);
            pb.line_to(13.0, 18.0);
            pb.line_to(13.0, 6.0);
            pb.line_to(8.0, 10.0);
            pb.close();
            paths.extend(pb.finish());
            if muted || level <= 0.005 {
                // dấu X: tắt tiếng
                for (x0, y0, x1, y1) in [(16.0, 9.5, 21.0, 14.5), (21.0, 9.5, 16.0, 14.5)] {
                    let mut pb = PathBuilder::new();
                    pb.move_to(x0, y0);
                    pb.line_to(x1, y1);
                    paths.extend(pb.finish());
                }
            } else {
                // 1–3 vạch sóng theo mức âm lượng (như taskbar gốc)
                let waves: &[(f32, f32, f32)] = if level < 0.34 {
                    &[(13.85, 4.0, 0.848)]
                } else if level < 0.67 {
                    &[(13.85, 4.0, 0.848), (13.19, 8.0, 0.758)]
                } else {
                    &[(13.85, 4.0, 0.848), (13.19, 8.0, 0.758), (12.6, 11.5, 0.70)]
                };
                for &(cx, r, a) in waves {
                    let mut pb = PathBuilder::new();
                    arc(&mut pb, cx, 12.0, r, -a, a, true);
                    paths.extend(pb.finish());
                }
            }
        }
        Icon::Battery(pct) => {
            paths.extend(round_rect_path(2.5, 7.0, 17.0, 10.0, 2.5));
            let mut pb = PathBuilder::new();
            pb.move_to(21.5, 11.0);
            pb.line_to(21.5, 13.0);
            paths.extend(pb.finish());
            let w = 13.0 * pct as f32 / 100.0;
            if w > 0.5 {
                fills.extend(round_rect_path(4.5, 9.0, w, 6.0, 1.0));
            }
        }
    }
    for p in paths {
        if let Some(p) = p.transform(t) {
            cv.stroke(&p, solid(col), sw);
        }
    }
    for p in fills {
        if let Some(p) = p.transform(t) {
            cv.fill(&p, solid(col));
        }
    }
}

impl Bar {
    pub fn new(el: &ActiveEventLoop, window: Arc<Window>, kind: Kind, backdrop: Option<Pixmap>, monitor: &Monitor) -> Option<Self> {
        let hwnd = hwnd_of(&window)?;
        let _ = el;
        let fhwnd = shell::create_popup()?;
        let chwnd = shell::create_popup()?;
        let mhwnd = shell::create_popup()?;
        let phwnd = shell::create_preview()?;
        shell::watch_foreground();
        let mon = monitor.rect;
        let k = window.scale_factor() as f32 * SCALE;
        let wpx = mon.right - mon.left;
        let hpx = (WIN_H * k).ceil() as i32;
        shell::make_bar_window(hwnd, mon.left, mon.bottom - hpx, wpx, hpx);
        shell::appbar_register(hwnd, mon, (RESERVE * k).round() as i32);
        let f = |b: &[u8]| FontVec::try_from_vec(b.to_vec()).ok();
        let fonts = match (f(widget::BE_VIETNAM_REGULAR), f(widget::BE_VIETNAM_SEMIBOLD)) {
            (Some(regular), Some(semibold)) => Some(Fonts { regular, semibold }),
            _ => None,
        };
        let now = Instant::now();
        let mut b = Bar {
            primary: monitor.primary,
            window,
            monitor: monitor.name.clone(),
            hwnd,
            kind,
            k,
            mon,
            wpx,
            hpx,
            layered: shell::Layered::new(wpx, hpx),
            fonts,
            backdrop,
            apps: Vec::new(),
            fg: 0,
            media: media::Media::start(),
            minfo: Default::default(),
            hover: None,
            cursor: (0.0, 0.0),
            hits: Vec::new(),
            hidden: false,
            t0: now,
            next_frame: now,
            battery: None,
            tray_pinned: Vec::new(),
            tray_hidden: Vec::new(),
            tray_gen: u64::MAX,
            promoted: shell::promoted_exes(),
            tray_check: now,
            pins: Vec::new(),
            pins_check: now - Duration::from_secs(60),
            pins_sig: Vec::new(),
            pins_rx: None,
            appbar_tries: 0,
            exe_cache: std::collections::HashMap::new(),
            aumid_cache: std::collections::HashMap::new(),
            audio: shell::Audio::new(),
            vol: (0.0, false),
            audio_check: now,
            preview: preview::Preview::new(phwnd),
            preview_at: None,
            preview_hide_at: None,
            tile_pos: Vec::new(),
            tile_slide: std::collections::HashMap::new(),
            tile_born: std::collections::HashMap::new(),
            tile_dying: Vec::new(),
            tile_icons: std::collections::HashMap::new(),
            island_w: (0.0, 0.0, now),
            chev: (0.0, 0.0, now),
            drag: None,
            tiles_x0: 0.0,
            start_cache: std::collections::HashMap::new(),
            order_gen: 0,
            native_gen: 0,
            tiles_animating: false,
            menu: menu::Menu::new(mhwnd),
            menu_ctx: MenuCtx::None,
            jump: Vec::new(),
            action: None,
            outside_closed: None,
            cal: Calendar { hwnd: chwnd, layered: None, open: false, x: 0, y: 0, w: 0, h: 0, month_off: 0, hover: None, close_at: None, fade: None },
            work: None,
            tray_pos: Vec::new(),
            chevron_x: 0.0,
            clock_x: 0.0,
            flyout: Flyout { hwnd: fhwnd, layered: None, open: false, x: 0, y: 0, w: 0, h: 0, cols: 1, hover: None, hits: Vec::new(), close_at: None, pending: false, fade: None },
            peek_until: None,
            net: shell::network(),
            net_check: now,
            tray: shell::tray_wnd().map(|h| h.0 as isize),
            closed: false,
            cache: None,
            dirty: true,
            anim: Anim::default(),
            sig: String::new(),
        };
        b.refresh();
        b.tick();
        shell::show(hwnd, true);
        crate::log(&format!("bật taskbar riêng ({}), {}x{} px", kind.name(), wpx, hpx));
        Some(b)
    }

    pub fn set_kind(&mut self, kind: Kind) {
        if kind != self.kind {
            self.kind = kind;
            self.dirty = true;
        }
    }

    pub fn set_backdrop(&mut self, bd: Option<Pixmap>) {
        self.backdrop = bd;
        self.dirty = true;
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Cập nhật danh sách app, nhạc, pin; tránh ra khi có app toàn màn hình; giữ taskbar gốc ẩn.
    pub fn refresh(&mut self) {
        // Icon đúng một cỡ chuẩn có sẵn trong file icon (24 px ở 100% như taskbar gốc) → không bị co giãn, nét.
        let want = 28.0 * self.k;
        let px = [16u32, 20, 24, 32, 40, 48, 64].into_iter().min_by(|a, b| (*a as f32 - want).abs().total_cmp(&(*b as f32 - want).abs()).then(b.cmp(a))).unwrap_or(24);
        // Danh sách ghim: đọc ở luồng riêng (lấy icon tốn vài chục ms) và chỉ khi nó đổi
        if let Some(rx) = &self.pins_rx {
            match rx.try_recv() {
                Ok(pins) => {
                    let names = |v: &[shell::Pin]| v.iter().map(|p| format!("{} [{}|{}]", p.name, p.aumid, p.target)).collect::<Vec<_>>().join(", ");
                    if names(&pins) != names(&self.pins) {
                        crate::log(&format!("app ghim: {}", names(&pins)));
                    }
                    self.pins = pins;
                    self.pins_rx = None;
                    self.dirty = true;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.pins_rx = None,
                Err(_) => {}
            }
        }
        if shell::pins_changed() {
            self.pins_check = Instant::now() - Duration::from_secs(60);
        }
        if self.pins_rx.is_none() && self.pins_check.elapsed() > Duration::from_secs(2) {
            self.pins_check = Instant::now();
            let t = Instant::now();
            let sig = shell::pins_signature();
            crate::prof_add("    └ kiểm tra app ghim", t);
            if sig != self.pins_sig {
                self.pins_sig = sig;
                let (tx, rx) = std::sync::mpsc::channel();
                self.pins_rx = Some(rx);
                let size = px as i32;
                let _ = std::thread::Builder::new().name("app-ghim".into()).spawn(move || {
                    let _ = tx.send(shell::pinned_apps(size));
                });
            }
        }
        let t = Instant::now();
        let list = shell::list_windows();
        crate::prof_add("    └ danh sách cửa sổ", t);
        self.exe_cache.retain(|h, _| list.iter().any(|w| w.hwnd == *h));
        self.aumid_cache.retain(|h, _| list.iter().any(|w| w.hwnd == *h));
        // (cửa sổ, chương trình, tiêu đề, ID app) theo thứ tự mới dùng trước
        let wins: Vec<(isize, String, String, String)> = list
            .iter()
            .map(|w| {
                let exe = self.exe_cache.entry(w.hwnd).or_insert_with(|| shell::exe_path(w.hwnd)).clone();
                // ID app như taskbar gốc: ID gắn trên cửa sổ, không thì ID của gói (app Store/MSIX),
                // không thì ID trong lối tắt Start Menu của chương trình
                let id = self
                    .aumid_cache
                    .entry(w.hwnd)
                    .or_insert_with(|| {
                        let a = jumplist::window_aumid(w.hwnd);
                        if a.is_empty() { shell::package_aumid(w.hwnd).unwrap_or_default() } else { a }
                    })
                    .clone();
                let id = if id.is_empty() { shell::start_menu_aumid(&exe).unwrap_or_default() } else { id };
                (w.hwnd, exe, w.title.clone(), id)
            })
            .collect();
        // Cửa sổ có thuộc app ghim này không: cùng ID app (nếu cả hai có), không thì cùng chương trình
        let belongs = |p: &shell::Pin, w: &(isize, String, String, String)| {
            if !p.aumid.is_empty() && !w.3.is_empty() {
                p.aumid.eq_ignore_ascii_case(&w.3)
            } else {
                !p.target.is_empty() && w.1 == p.target
            }
        };
        // Giữ icon đã lấy và thứ tự cũ của các ô
        let mut old: Vec<Tile> = std::mem::take(&mut self.apps);
        let old_order: Vec<(String, String)> = old.iter().map(|t| (t.key.clone(), t.alias.clone())).collect();
        // Thứ tự cũ chỉ đáng tin khi lần trước đã có danh sách ghim (lần đầu ghim còn đang được đọc)
        let had_pins = old.iter().any(|t| t.lnk.is_some()) || self.pins.is_empty();
        // Khoá của một cửa sổ đang chạy (giống cách gộp ô): ID app, không thì chương trình
        let run_key = |h: isize, exe: &str, id: &str| {
            if !id.is_empty() {
                format!("id:{}", id.to_lowercase())
            } else if exe.is_empty() {
                format!("win:{h}")
            } else {
                format!("exe:{exe}")
            }
        };
        let mut take_icon = |key: &str| old.iter_mut().find(|t| t.key == key).and_then(|t| t.icon.take());
        let mut used: Vec<isize> = Vec::new();
        let mut apps: Vec<Tile> = Vec::new();
        let to_pm = |v: Vec<u8>| Pixmap::from_vec(v, IntSize::from_wh(px, px)?);
        // 1) App đã ghim, đúng thứ tự như taskbar gốc; mọi cửa sổ của nó gộp vào ô này
        for (i, p) in self.pins.iter().enumerate() {
            let key = format!("pin:{}", p.lnk.to_lowercase());
            let mine: Vec<&(isize, String, String, String)> = wins.iter().filter(|w| !used.contains(&w.0) && belongs(p, w)).collect();
            let icon = take_icon(&key).or_else(|| p.icon.clone().and_then(to_pm));
            match mine.first() {
                Some(first) => {
                    used.extend(mine.iter().map(|w| w.0));
                    let alias = run_key(first.0, &first.1, &first.3);
                    apps.push(Tile { hwnd: first.0, wins: mine.iter().map(|w| w.0).collect(), key, alias, title: first.2.clone(), icon, lnk: Some(p.lnk.clone()) });
                }
                None => apps.push(Tile { hwnd: -(i as isize) - 1, wins: Vec::new(), key, alias: String::new(), title: p.name.clone(), icon, lnk: Some(p.lnk.clone()) }),
            }
        }
        // 2) App khác đang mở: các cửa sổ cùng chương trình gộp chung một ô
        let mut groups: Vec<Tile> = Vec::new();
        for (h, exe, title, id) in &wins {
            if used.contains(h) {
                continue;
            }
            let key = run_key(*h, exe, id);
            if let Some(g) = groups.iter_mut().find(|g| g.key == key) {
                g.wins.push(*h);
                continue;
            }
            let icon = take_icon(&key).or_else(|| shell::window_icon_for(*h, exe, id, px as i32).and_then(to_pm));
            groups.push(Tile { hwnd: *h, wins: vec![*h], alias: key.clone(), key, title: title.clone(), icon, lnk: None });
        }
        // Như taskbar gốc: app đang mở (chưa ghim) xếp theo thứ tự được mở, không theo lần dùng gần nhất
        self.start_cache.retain(|h, _| list.iter().any(|w| w.hwnd == *h));
        for g in &groups {
            for h in &g.wins {
                self.start_cache.entry(*h).or_insert_with(|| shell::process_started(*h));
            }
        }
        let started = |g: &Tile| g.wins.iter().filter_map(|h| self.start_cache.get(h).copied()).filter(|t| *t > 0).min().unwrap_or(u64::MAX);
        groups.sort_by_key(|g| started(g));
        apps.extend(groups);
        // Thứ tự ổn định như taskbar gốc: ô đã có giữ nguyên chỗ (kể cả khi vừa ghim / bỏ ghim một
        // app đang mở – nhận ra nhờ khoá theo app đang chạy); ô mới đứng ngay sau ô đứng trước nó
        // (app ghim mới vào đúng chỗ trong nhóm ghim, app mới mở thì ở cuối).
        if !old_order.is_empty() && had_pins {
            let mut last = -1.0f64;
            let mut fresh = 0.0f64;
            let mut fresh_n = || {
                fresh += 1.0;
                fresh
            };
            let mut ranked: Vec<(f64, Tile)> = apps
                .into_iter()
                .map(|t| {
                    let pos = old_order.iter().position(|(k, a)| *k == t.key || (!t.alias.is_empty() && *a == t.alias));
                    let r = match pos {
                        Some(p) => p as f64,
                        // App mới mở (chưa ghim): luôn ở cuối đảo như taskbar gốc
                        None if t.lnk.is_none() => 1.0e9 + fresh_n(),
                        // App ghim mới: ngay sau ô đứng trước nó trong nhóm ghim
                        None => last + 0.001,
                    };
                    if r < 1.0e9 {
                        last = r;
                    }
                    (r, t)
                })
                .collect();
            ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
            apps = ranked.into_iter().map(|(_, t)| t).collect();
        }
        // Thứ tự của taskbar gốc (đọc qua UI Automation) vừa đổi, hoặc lần đầu: xếp theo đúng như vậy
        // (áp ở mọi lần làm mới: app mở sau cũng vào đúng chỗ như taskbar gốc; ô không có trên
        // taskbar gốc giữ thứ tự tương đối và đứng cuối)
        let (ngen, native) = shell::native_order();
        if !native.is_empty() {
            let rank = |t: &Tile| native.iter().position(|a| *a == tile_appid(t, &self.pins)).unwrap_or(usize::MAX);
            apps.sort_by_key(|t| rank(t));
            if ngen != self.native_gen {
                self.native_gen = ngen;
                self.dirty = true;
                if std::env::var_os("SEASON_ORDER_DEBUG").is_some() {
                    let d: Vec<String> = apps.iter().map(|t| format!("{} [{}] #{}", t.title, tile_appid(t, &self.pins), rank(t))).collect();
                    crate::log(&format!("xếp theo taskbar gốc: {}", d.join(" | ")));
                }
            }
        }
        // Thứ tự vừa được kéo đổi ở taskbar màn hình khác: xếp theo cho giống nhau
        let (gen, keys) = shell::tile_order();
        if gen != self.order_gen {
            self.order_gen = gen;
            let rank = |t: &Tile| keys.iter().position(|k| *k == t.key).unwrap_or(usize::MAX);
            apps.sort_by_key(|t| rank(t));
            self.dirty = true;
        }
        self.apps = apps;
        // Âm lượng (loa mặc định đổi thì lấy lại)
        if self.audio.is_none() || self.audio_check.elapsed() > Duration::from_secs(15) {
            self.audio_check = Instant::now();
            self.audio = shell::Audio::new();
        }
        if let Some(a) = &self.audio {
            self.vol = a.get();
        }
        if self.cal.open && self.cal.close_at.is_some_and(|t| Instant::now() >= t) {
            self.close_calendar();
        }
        self.fg = shell::foreground();
        let t = Instant::now();
        self.minfo = self.media.get();
        self.battery = shell::battery_percent();
        crate::prof_add("    └ nhạc + pin", t);
        if self.tray_check.elapsed() > Duration::from_secs(5) {
            self.tray_check = Instant::now();
            self.promoted = shell::promoted_exes();
            self.tray_gen = u64::MAX;
        }
        if systray::generation() != self.tray_gen {
            let t = Instant::now();
            self.sync_tray();
            crate::prof_add("    └ icon khay", t);
        }
        if self.flyout.open && self.flyout.close_at.is_some_and(|t| Instant::now() >= t) {
            self.close_flyout();
        }
        // Taskbar gốc lỡ hiện lại (ví dụ Explorer tự bật lên): ẩn đi.
        if shell::windows_taskbar_visible() {
            shell::hide_windows_taskbar();
        }
        if self.net_check.elapsed() > Duration::from_secs(2) {
            let t = Instant::now();
            self.net = shell::network_cached();
            crate::prof_add("    └ mạng", t);
            self.net_check = Instant::now();
            // Windows lỡ bỏ chỗ giữ cho thanh (ví dụ Explorer sắp xếp lại lúc khởi động): giữ lại.
            let reserve = (RESERVE * self.k).round() as i32;
            if !self.hidden && shell::work_bottom(self.mon).is_some_and(|b| (b - (self.mon.bottom - reserve)).abs() > 1) {
                shell::set_work_area(self.mon, reserve);
                if self.appbar_tries < 3 {
                    self.appbar_tries += 1;
                    crate::log(&format!("đặt lại vùng làm việc cho taskbar riêng, đáy: {:?}", shell::work_bottom(self.mon)));
                }
            }
        }

        let peeking = self.peek_until.is_some_and(|t| Instant::now() < t);
        if self.peek_until.is_some() && !peeking {
            self.peek_until = None;
            shell::peek_windows_taskbar(false);
        }
        let hide = peeking || shell::fullscreen_active(self.mon);
        self.set_hidden(hide);
        self.fix_style();
        // Chỉ vẽ lại phần tĩnh khi thực sự có gì đổi.
        let now = local_time();
        let mut sig = format!("{}|{}|{}|{:?}|{:?}|{}:{}|{}|{}|{}|", self.fg, self.minfo.title, self.minfo.playing, self.battery, self.net, now.hour, now.minute, self.tray_gen, (self.vol.0 * 100.0).round(), self.vol.1);
        for (t, _) in &self.tray_pinned {
            sig += &t.tip;
        }
        for a in &self.apps {
            sig += &format!("{}:{};", a.hwnd, a.title);
        }
        if sig != self.sig {
            self.sig = sig;
            self.dirty = true;
        }
        // Explorer khởi động lại: đăng ký giữ chỗ lại.
        let tray = shell::tray_wnd().map(|h| h.0 as isize);
        if tray != self.tray {
            self.tray = tray;
            shell::hide_windows_taskbar();
            shell::appbar_register(self.hwnd, self.mon, (RESERVE * self.k).round() as i32);
        }
    }

    /// Cửa sổ taskbar lỡ bị đặt lại kiểu thường (có thanh tiêu đề): đặt lại và vẽ lại ngay.
    fn fix_style(&mut self) {
        if !shell::bar_style_ok(self.hwnd) {
            crate::log("taskbar bị đổi kiểu cửa sổ, đặt lại kiểu trong suốt");
            shell::make_bar_window(self.hwnd, self.mon.left, self.mon.bottom - self.hpx, self.wpx, self.hpx);
            if self.hidden {
                shell::show(self.hwnd, false);
            }
            self.cache = None;
            self.dirty = true;
            self.next_frame = Instant::now();
        }
    }

    /// Ẩn/hiện thanh (khi có app toàn màn hình trên màn hình này).
    fn set_hidden(&mut self, hide: bool) {
        if hide != self.hidden {
            self.hidden = hide;
            if hide {
                self.preview.close();
                self.menu.close();
                self.close_calendar();
                self.close_flyout();
            }
            shell::show(self.hwnd, !hide);
            self.fix_style();
            self.dirty = true;
            self.next_frame = Instant::now();
        }
    }

    /// Cửa sổ vừa đổi (vào/ra toàn màn hình): xét lại ngay có cần nhường chỗ không.
    pub fn check_fullscreen(&mut self) {
        let peeking = self.peek_until.is_some_and(|t| Instant::now() < t);
        let hide = peeking || shell::fullscreen_active(self.mon);
        self.set_hidden(hide);
    }

    /// Xử lý chuột của các cửa sổ nổi ngay (không chờ tới nhịp vẽ sau).
    pub fn poll_popups(&mut self) {
        self.poll_flyout();
        self.poll_preview();
        // Icon khay đổi (ví dụ bấm EVKey đổi V/E) hoặc đổi cửa sổ đang chọn: vẽ lại ngay
        if systray::generation() != self.tray_gen {
            self.sync_tray();
            self.dirty = true;
            self.next_frame = Instant::now();
        }
        if shell::foreground_changed() {
            let fg = shell::foreground();
            if fg != self.fg {
                self.fg = fg;
                self.dirty = true;
                self.next_frame = Instant::now();
            }
        }
        if self.animate_popups() {
            self.next_frame = self.next_frame.min(Instant::now() + Duration::from_millis(8));
        }
    }

    /// Chạy một bước hiệu ứng của các cửa sổ nổi. Trả về true nếu còn hiệu ứng đang chạy.
    fn animate_popups(&mut self) -> bool {
        let kk = self.k / SCALE;
        let mut busy = shell::fade_step(&mut self.flyout.fade, &mut self.flyout.layered, self.flyout.hwnd, kk);
        busy |= shell::fade_step(&mut self.cal.fade, &mut self.cal.layered, self.cal.hwnd, kk);
        busy |= self.menu.animate();
        busy |= self.preview.animate();
        busy
    }

    /// Đọc lại icon khay: icon ghim ra ngoài và icon ẩn, xếp đúng thứ tự Windows lưu.
    fn sync_tray(&mut self) {
        let gen = systray::generation();
        self.tray_gen = gen;
        let order = shell::tray_order();
        let rank = |t: &systray::TrayIcon| {
            order
                .iter()
                .position(|(g, exe, uid)| match (g, &t.guid) {
                    (Some(g), Some(tg)) => *g == shell::guid_string(tg),
                    (None, None) => *exe == t.exe && *uid == t.uid,
                    _ => false,
                })
                .unwrap_or(usize::MAX)
        };
        let (mut pinned, mut hidden) = (Vec::new(), Vec::new());
        let mut list = systray::icons();
        // Thêm icon mà Windows có nhưng không gửi sang (ví dụ NVIDIA), để giống hệt bảng gốc
        let received: Vec<(String, u32)> = list.iter().map(|t| (t.exe.clone(), t.uid)).collect();
        for (exe, uid, tip, rgba, promoted) in shell::tray_proxies(&received) {
            if promoted {
                self.promoted.insert(exe.clone());
            }
            list.push(systray::TrayIcon { hwnd: 0, uid, guid: None, callback: 0, version: 0, tip, rgba: Some(rgba), hidden: false, exe });
        }
        list.sort_by_key(|t| rank(t));
        for t in list {
            if t.hidden {
                continue;
            }
            let Some(pm) = t.rgba.clone().and_then(|v| Pixmap::from_vec(v, IntSize::from_wh(32, 32)?)) else { continue };
            if self.promoted.contains(&t.exe) {
                pinned.push((t, pm));
            } else {
                hidden.push((t, pm));
            }
        }
        self.tray_pinned = pinned;
        self.tray_hidden = hidden;
        if self.flyout.open {
            self.draw_flyout();
        }
    }

    /// Rê chuột sang ô khác: hẹn mở / đổi / đóng ảnh xem trước như taskbar gốc.
    fn hover_changed(&mut self, h: Option<Hit>) {
        let now = Instant::now();
        match h {
            Some(Hit::App(t)) if t > 0 => {
                self.preview_hide_at = None;
                if self.preview.open {
                    if self.preview.tile != t {
                        self.open_preview(t);
                    }
                } else {
                    self.preview_at = Some((now + Duration::from_millis(450), t));
                    self.next_frame = self.next_frame.min(now + Duration::from_millis(450));
                }
            }
            _ => {
                self.preview_at = None;
                if self.preview.open {
                    self.preview_hide_at = Some(now + Duration::from_millis(350));
                    self.next_frame = self.next_frame.min(now + Duration::from_millis(350));
                }
            }
        }
    }

    fn open_preview(&mut self, t: isize) {
        if self.menu.open {
            return;
        }
        let Some(tile) = self.apps.iter().find(|a| a.hwnd == t) else { return };
        let Some((x, _)) = self.tile_pos.iter().find(|(_, h)| *h == t) else { return };
        let wins = tile.wins.clone();
        let icon = tile.icon.clone();
        let (cx, top) = self.screen_pt(x + 21.0, TOP);
        let lk = look(self.kind);
        let theme = menu::Theme { tint: lk.bg, border: lk.border, fg: lk.fg, acc: lk.acc, snow: false };
        self.preview.show(t, &wins, icon, cx, top, self.mon, self.k / SCALE, Some(theme));
        self.dirty = true;
    }

    fn poll_preview(&mut self) {
        for e in shell::take_popup_events(self.preview.hwnd) {
            match e {
                shell::PopupEvent::Move(..) => self.preview_hide_at = None,
                shell::PopupEvent::Leave => self.preview_hide_at = Some(Instant::now() + Duration::from_millis(350)),
                _ => {}
            }
            match self.preview.mouse(e) {
                Some(preview::Cmd::Activate(h)) => shell::bring(h),
                Some(preview::Cmd::Close(h)) => {
                    shell::close_window(h);
                    self.dirty = true;
                }
                None => {}
            }
        }
        let now = Instant::now();
        if self.preview.animate() {
            self.next_frame = self.next_frame.min(now + Duration::from_millis(8));
        }
        if let Some((at, t)) = self.preview_at {
            if now >= at {
                self.preview_at = None;
                if self.hover == Some(Hit::App(t)) {
                    self.open_preview(t);
                }
            } else {
                self.next_frame = self.next_frame.min(at);
            }
        }
        if let Some(at) = self.preview_hide_at {
            if now >= at {
                self.preview_hide_at = None;
                let on_tile = matches!(self.hover, Some(Hit::App(t)) if t == self.preview.tile);
                if !self.preview.inside && !on_tile {
                    self.preview.close();
                }
            } else {
                self.next_frame = self.next_frame.min(at);
            }
        }
    }

    pub fn tick(&mut self) {
        self.poll_flyout();
        let now = Instant::now();
        if self.hidden {
            self.next_frame = now + Duration::from_millis(500);
            return;
        }
        if self.flyout.open && self.flyout.pending {
            self.flyout.pending = false;
            self.draw_flyout();
            shell::show(self.flyout.hwnd, true);
        }
        if self.tiles_animating {
            self.dirty = true;
        }
        let redraw = self.dirty || self.cache.is_none();
        if redraw {
            self.dirty = false;
            self.draw();
        }
        if redraw {
            self.present_frame_part(true);
        } else if self.anim.any() {
            self.present_frame_part(false);
        }
        // Không có gì chuyển động thì gần như không vẽ gì.
        self.next_frame = now + Duration::from_millis(if self.anim.any() { 1000 / FPS } else { 250 });
        if self.tiles_animating || self.preview.animating() || self.menu.animating() || self.flyout.fade.is_some() || self.cal.fade.is_some() {
            self.next_frame = now + Duration::from_millis(8);
        }
    }

    /// Ghép ảnh tĩnh với phần chuyển động rồi đẩy lên màn hình.
    /// Vùng (pixel) mà các hiệu ứng chuyển động có thể vẽ tới.
    fn anim_rect(&self) -> Option<(i32, i32, i32, i32)> {
        let a = self.anim;
        let mut r: Option<(f32, f32, f32, f32)> = None;
        let mut add = |x0: f32, y0: f32, x1: f32, y1: f32| {
            r = Some(match r {
                Some(o) => (o.0.min(x0), o.1.min(y0), o.2.max(x1), o.3.max(y1)),
                None => (x0, y0, x1, y1),
            });
        };
        if let Some((x, y, _)) = a.eq {
            add(x - 2.0, y - 23.0, x + 30.0, y + 2.0);
        }
        if let Some((x, y, w)) = a.lights {
            add(x - 2.0, y - 14.0, x + w + 2.0, y + 12.0);
        }
        if let Some((x, y)) = a.lantern {
            add(x - 14.0, y - 12.0, x + 60.0, y + 72.0);
        }
        let k = self.k;
        r.map(|(x0, y0, x1, y1)| ((x0 * k).floor() as i32 - 1, (y0 * k).floor() as i32 - 1, (x1 * k).ceil() as i32 + 1, (y1 * k).ceil() as i32 + 1))
    }

    /// Ghép ảnh tĩnh với hiệu ứng. `full` = đẩy cả thanh (sau khi vẽ lại phần tĩnh);
    /// còn lại chỉ ghép và đẩy vùng có hiệu ứng (rẻ hơn nhiều).
    fn present_frame_part(&mut self, full: bool) {
        let Some(cache) = &self.cache else { return };
        let t = self.t0.elapsed().as_secs_f32();
        let rect = if full { None } else { self.anim_rect() };
        let (cw, ch) = (cache.width() as i32, cache.height() as i32);
        let mut pm = match self.work.take() {
            Some(mut w) if w.width() == cache.width() && w.height() == cache.height() => {
                match rect {
                    // Chỉ chép lại vùng hiệu ứng từ ảnh tĩnh, phần còn lại của bộ đệm vẫn đúng
                    Some((x0, y0, x1, y1)) => {
                        let (x0, y0, x1, y1) = (x0.clamp(0, cw), y0.clamp(0, ch), x1.clamp(0, cw), y1.clamp(0, ch));
                        let src = cache.data();
                        let dst = w.data_mut();
                        for row in y0..y1 {
                            let a = ((row * cw + x0) * 4) as usize;
                            let b = ((row * cw + x1) * 4) as usize;
                            dst[a..b].copy_from_slice(&src[a..b]);
                        }
                    }
                    None => w.data_mut().copy_from_slice(cache.data()),
                }
                w
            }
            _ => cache.clone(),
        };
        let _ = &mut pm;
        let mut cv = Canvas { pm, base: Transform::from_scale(self.k, self.k) };
        let a = self.anim;
        if let Some((x, y, col)) = a.eq {
            for (i, ph) in [0.0f32, -0.3, -0.6, -0.15].iter().enumerate() {
                let bh = 5.0 + 15.0 * (0.5 - 0.5 * ((t + ph) * std::f32::consts::TAU).cos());
                cv.round_rect(x + i as f32 * 7.0, y - bh, 4.0, bh, 2.0, solid(ca(col[0], col[1], col[2], 1.0)));
            }
        }
        if let Some((x, y, w)) = a.lights {
            self.bulbs(&mut cv, x, y, w, t);
        }
        if let Some((x, y)) = a.lantern {
            self.star_lantern(&mut cv, x, y, t);
        }
        if let Some(l) = &mut self.layered {
            match rect {
                Some(r) => l.present_rect(self.hwnd, self.mon.left, self.mon.bottom - self.hpx, cv.pm.data(), r),
                None => l.present(self.hwnd, self.mon.left, self.mon.bottom - self.hpx, cv.pm.data()),
            }
        }
        self.work = Some(cv.pm);
    }

    pub fn on_event(&mut self, ev: &WindowEvent) -> Option<BarAction> {
        match ev {
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                // Bắt đầu có thể kéo ô app (chỉ thành kéo khi rê quá vài điểm ảnh)
                if let Some(Hit::App(h)) = self.hit_at(self.cursor) {
                    let x = self.tile_pos.iter().find(|(_, hw)| *hw == h).map(|(x, _)| *x);
                    if let (Some(t), Some(x)) = (self.apps.iter().find(|a| a.hwnd == h), x) {
                        self.drag = Some(Drag { key: t.key.clone(), press_x: self.cursor.0, grab: self.cursor.0 - x, active: false });
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = (position.x as f32 / self.k, position.y as f32 / self.k);
                if self.drag_move(p) {
                    self.cursor = p;
                    return None;
                }
                let h = self.hit_at(p);
                if h != self.hover {
                    self.hover_changed(h);
                    self.hover = h;
                    self.dirty = true;
                    self.next_frame = Instant::now();
                }
                self.cursor = p;
            }
            WindowEvent::CursorLeft { .. } => {
                // rời thanh khi đang kéo: thả ô tại chỗ đang đứng
                if self.drag.take().is_some_and(|d| d.active) {
                    self.finish_drag();
                }
                self.hover_changed(None);
                if self.hover.is_some() {
                    self.hover = None;
                    self.dirty = true;
                    self.next_frame = Instant::now();
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.preview_at = None;
                self.preview.close();
                // vừa kéo đổi chỗ xong: thả ô (trượt vào đúng chỗ), không tính là bấm
                if let Some(d) = self.drag.take() {
                    if d.active {
                        self.finish_drag();
                        return None;
                    }
                }
                match self.hit_at(self.cursor)? {
                    Hit::Start => shell::press(&[shell::KEY_WIN]),
                    Hit::App(h) => match self.apps.iter().find(|a| a.hwnd == h) {
                        // App đã ghim nhưng chưa mở: mở app
                        Some(t) if h < 0 => {
                            if let Some(l) = &t.lnk {
                                shell::launch(l);
                            }
                        }
                        // Nhiều cửa sổ: đang ở app này thì chuyển lần lượt sang cửa sổ khác của nó
                        Some(t) if t.wins.len() > 1 => {
                            if t.wins.contains(&self.fg) {
                                shell::activate(*t.wins.last().unwrap_or(&h));
                            } else {
                                shell::activate(t.wins[0]);
                            }
                        }
                        _ => shell::activate(h),
                    },
                    Hit::Media => self.media.toggle(),
                    Hit::Settings => return Some(BarAction::OpenSettings),
                    // Bảng icon ẩn của chính taskbar này (theo chủ đề)
                    Hit::Peek => {
                        if !self.just_closed(1) {
                            self.toggle_flyout();
                        }
                    }
                    // Bấm vào icon khay: gửi thẳng cho app như taskbar gốc
                    Hit::Tray(i) => self.click_tray(i, false),
                    Hit::Net | Hit::Volume => shell::press(&[shell::KEY_WIN, shell::KEY_A]),
                    // Lịch tháng của chính taskbar này
                    Hit::Clock => {
                        if !self.just_closed(2) {
                            self.toggle_calendar();
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Right, .. } => match {
                self.preview_at = None;
                self.preview.close();
                self.hit_at(self.cursor)
            } {
                Some(Hit::Tray(i)) => self.click_tray(i, true),
                Some(Hit::App(h)) => self.app_menu(h),
                other => return self.context_menu(other),
            },
            // Cuộn chuột trên nút loa: tăng/giảm âm lượng 2% mỗi nấc (như taskbar gốc)
            WindowEvent::MouseWheel { delta, .. } => {
                if self.hit_at(self.cursor) == Some(Hit::Volume) {
                    let notches = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                        winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 120.0,
                    };
                    if notches != 0.0 {
                        if self.audio.is_none() {
                            self.audio = shell::Audio::new();
                        }
                        if let Some(a) = &self.audio {
                            self.vol = a.step(notches * 0.02);
                            self.dirty = true;
                            self.next_frame = Instant::now();
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// Thả ô sau khi kéo: báo thứ tự mới cho taskbar ở màn hình khác, và ghi thứ tự app ghim cho
    /// taskbar Windows (như khi kéo trên taskbar gốc).
    fn finish_drag(&mut self) {
        self.dirty = true;
        self.next_frame = Instant::now();
        let keys: Vec<String> = self.apps.iter().map(|t| t.key.clone()).collect();
        shell::publish_tile_order(keys);
        // thứ tự vừa kéo thành thứ tự chung (màn hình khác và lần mở app sau đều theo)
        shell::set_native_order(self.apps.iter().map(|t| tile_appid(t, &self.pins)).collect());
        self.native_gen = shell::native_order().0;
        // tự mình đã có thứ tự này rồi
        self.order_gen = shell::tile_order().0;
    }

    /// Rê chuột khi đang giữ nút trái trên một ô app: kéo ô đó đổi chỗ với các ô bên cạnh.
    /// Trả về true khi đang kéo (không xử lý rê chuột bình thường).
    fn drag_move(&mut self, p: (f32, f32)) -> bool {
        let Some(d) = &mut self.drag else { return false };
        if !d.active {
            if (p.0 - d.press_x).abs() < 6.0 {
                return false;
            }
            d.active = true;
            self.preview_at = None;
            self.preview.close();
        }
        let (key, grab) = (d.key.clone(), d.grab);
        let n = self.tile_pos.len();
        if let Some(from) = self.apps.iter().position(|a| a.key == key) {
            if n > 0 && from < n {
                // chỗ thả: ô có tâm gần tâm ô đang kéo nhất
                let x = p.0 - grab;
                let to = (((x - self.tiles_x0) / 50.0).round().max(0.0) as usize).min(n - 1);
                if to != from {
                    let t = self.apps.remove(from);
                    self.apps.insert(to, t);
                }
            }
        }
        self.dirty = true;
        self.next_frame = Instant::now();
        true
    }

    /// Toạ độ màn hình (pixel) của điểm (x, y) logic trên thanh.
    fn screen_pt(&self, x: f32, y: f32) -> (i32, i32) {
        (self.mon.left + (x * self.k) as i32, self.mon.bottom - self.hpx + (y * self.k) as i32)
    }

    /// Kích thước icon 16 px theo DPI của màn hình này.
    fn small_icon(&self, path: &str, idx: i32) -> menu::Ico {
        let px = (16.0 * self.k / SCALE).round() as u32;
        match shell::icon_at(path, idx, px as i32).and_then(|v| Pixmap::from_vec(v, IntSize::from_wh(px, px)?)) {
            Some(p) => menu::Ico::Image(p),
            None => menu::Ico::None,
        }
    }

    /// Mở menu ngay phía trên con trỏ.
    fn show_menu(&mut self, items: Vec<menu::Item>) {
        let (sx, top) = self.screen_pt(self.cursor.0, TOP);
        let k = self.k / SCALE;
        let lk = look(self.kind);
        let theme = menu::Theme { tint: lk.bg, border: lk.border, fg: lk.fg, acc: lk.acc, snow: self.kind == Kind::Noel };
        self.close_calendar();
        self.close_flyout();
        self.menu.show(items, sx, top, self.mon, k, Some(theme));
    }

    /// Menu chuột phải của app ở đảo giữa – giống taskbar gốc: jump list (nhóm riêng của app,
    /// Gần đây, Tác vụ), tên app (mở thêm), ghim/bỏ ghim, đóng cửa sổ.
    fn app_menu(&mut self, h: isize) {
        let Some(t) = self.apps.iter().find(|a| a.hwnd == h) else { return };
        let (title, lnk, running, wins) = (t.title.clone(), t.lnk.clone(), h > 0, t.wins.clone());
        let pinned_target = lnk.as_ref().and_then(|l| self.pins.iter().find(|p| &p.lnk == l)).map(|p| p.target.clone()).unwrap_or_default();
        let exe = if running { shell::exe_path(h) } else { pinned_target };
        let groups = jumplist::for_app(&exe, running.then_some(h), lnk.as_deref());
        let mut items = Vec::new();
        let mut jump = Vec::new();
        for g in groups {
            items.push(menu::Item::header(g.name));
            for e in g.entries {
                let icon = match &e.icon {
                    Some((p, i)) => self.small_icon(p, *i),
                    None => menu::Ico::None,
                };
                items.push(menu::Item::new(1000 + jump.len() as u32, e.title.clone(), icon));
                jump.push(e);
            }
        }
        if !items.is_empty() {
            items.push(menu::Item::sep());
        }
        // Tên app như taskbar gốc: tên lối tắt đã ghim, hoặc tên chương trình
        let name = lnk
            .as_ref()
            .and_then(|l| self.pins.iter().find(|p| &p.lnk == l).map(|p| p.name.clone()))
            .or_else(|| shell::exe_description(&exe))
            .unwrap_or(title);
        let icon_src = if !exe.is_empty() { exe.clone() } else { lnk.clone().unwrap_or_default() };
        let mut app_icon = self.small_icon(&icon_src, 0);
        // App Store đã ghim: icon theo ID của app
        if let (menu::Ico::None, Some(id)) = (&app_icon, lnk.as_ref().and_then(|l| l.strip_prefix(r"shell:AppsFolder\"))) {
            let px = (16.0 * self.k / SCALE).round() as u32;
            if let Some(pm) = shell::aumid_icon(id, px as i32).and_then(|v| Pixmap::from_vec(v, IntSize::from_wh(px, px)?)) {
                app_icon = menu::Ico::Image(pm);
            }
        }
        items.push(menu::Item::new(1, name, app_icon));
        // Không có Ghim / Bỏ ghim: Windows chặn app khác ghim vào taskbar (kể cả qua Explorer), còn bỏ ghim
        // bằng API thì Windows tự dọn danh sách và có thể mất ghim khác. Ghim / bỏ ghim trên taskbar gốc
        // hoặc Start Menu, taskbar của app đọc theo ngay.
        if wins.len() > 1 {
            items.push(menu::Item::new(4, menu::tr("Close all windows", "Đóng tất cả cửa sổ"), menu::Ico::Glyph('\u{E8BB}')));
        } else if running {
            items.push(menu::Item::new(4, menu::tr("Close window", "Đóng cửa sổ"), menu::Ico::Glyph('\u{E8BB}')));
        }
        self.menu_ctx = MenuCtx::App { hwnd: h, lnk, exe, wins };
        self.jump = jump;
        self.show_menu(items);
    }

    /// Menu chuột phải của các nút hệ thống và chỗ trống trên thanh (giống taskbar gốc).
    fn context_menu(&mut self, hit: Option<Hit>) -> Option<BarAction> {
        use menu::{tr, Ico, Item};
        let g = |c: char| Ico::Glyph(c);
        let items: Vec<Item> = match hit {
            Some(Hit::Volume) => vec![
                Item::new(13, tr("Troubleshoot sound problems", "Khắc phục sự cố âm thanh"), g('\u{E90F}')),
                Item::new(11, tr("Open volume mixer", "Mở bộ trộn âm lượng"), g('\u{E767}')),
                Item::sep(),
                Item::new(12, tr("Sound settings", "Cài đặt âm thanh"), g('\u{E713}')),
            ],
            Some(Hit::Net) => {
                let mut v = vec![
                    Item::new(21, tr("Diagnose network problems", "Chẩn đoán sự cố mạng"), g('\u{E90F}')),
                    Item::new(24, tr("Perform speed test", "Kiểm tra tốc độ mạng"), g('\u{EC4A}')),
                    Item::sep(),
                    Item::new(20, tr("Network and Internet settings", "Cài đặt mạng và Internet"), g('\u{E713}')),
                ];
                if self.battery.is_some() {
                    v.push(Item::new(23, tr("Power and battery settings", "Cài đặt nguồn và pin"), g('\u{E83F}')));
                }
                v
            }
            Some(Hit::Clock) => vec![
                Item::new(30, tr("Adjust date and time", "Điều chỉnh ngày và giờ"), g('\u{E787}')),
                Item::new(32, tr("Notification settings", "Cài đặt thông báo"), g('\u{EA8F}')),
            ],
            Some(Hit::Start) => {
                // Như chuột phải nút Start của Windows: menu Win+X
                shell::press(&[shell::KEY_WIN, shell::KEY_X]);
                return None;
            }
            Some(Hit::Media) => vec![
                Item::new(40, tr("Play / Pause", "Phát / Tạm dừng"), g('\u{E768}')),
                Item::sep(),
                Item::new(12, tr("Sound settings", "Cài đặt âm thanh"), g('\u{E713}')),
            ],
            // Chỗ trống, nút Season Desktop, nút ^: menu của taskbar
            _ => vec![
                Item::new(1, tr("Task Manager", "Trình quản lý tác vụ"), g('\u{E9D9}')),
                Item::new(3, tr("Taskbar settings", "Cài đặt thanh tác vụ"), g('\u{E713}')),
                Item::sep(),
                Item::new(2, "Season Desktop", g('\u{E771}')),
            ],
        };
        self.menu_ctx = MenuCtx::System;
        self.show_menu(items);
        None
    }

    /// Thực hiện lựa chọn trong menu.
    fn menu_command(&mut self, id: u32) {
        let run = |path: &str, args: &str| jumplist::run(&jumplist::Entry { title: String::new(), path: path.into(), args: args.into(), dir: String::new(), icon: None });
        if let MenuCtx::App { hwnd, lnk, exe, wins } = std::mem::replace(&mut self.menu_ctx, MenuCtx::None) {
            match id {
                1 => match &lnk {
                    Some(l) => shell::launch(l),
                    None => shell::launch(&exe),
                },
                4 => {
                    if wins.is_empty() {
                        shell::close_window(hwnd);
                    }
                    for w in wins {
                        shell::close_window(w);
                    }
                }
                n if n >= 1000 => {
                    if let Some(e) = self.jump.get((n - 1000) as usize) {
                        jumplist::run(e);
                    }
                }
                _ => {}
            }
        } else {
            match id {
                1 => shell::open("taskmgr.exe"),
                2 => self.action = Some(BarAction::OpenSettings),
                3 => shell::open("ms-settings:taskbar"),
                11 => shell::open("ms-settings:apps-volume"),
                12 => shell::open("ms-settings:sound"),
                13 => run("msdt.exe", "-id AudioPlaybackDiagnostic"),
                20 => shell::open("ms-settings:network"),
                21 => run("msdt.exe", "-id NetworkDiagnosticsNetworkAdapter"),
                23 => shell::open("ms-settings:batterysaver"),
                24 => shell::open("https://www.bing.com/search?q=Speed%20test"),
                30 => shell::open("ms-settings:dateandtime"),
                32 => shell::open("ms-settings:notifications"),
                40 => self.media.toggle(),
                _ => {}
            }
        }
        self.jump.clear();
        self.dirty = true;
        self.next_frame = Instant::now();
    }

    /// Lựa chọn trong menu cần app chính xử lý (mở cửa sổ cài đặt).
    pub fn take_action(&mut self) -> Option<BarAction> {
        self.action.take()
    }

    fn click_tray(&mut self, i: usize, right: bool) {
        if let (Some((t, _)), Some(x)) = (self.tray_pinned.get(i), self.tray_pos.get(i)) {
            let (sx, sy) = self.screen_pt(*x, TOP + BAR_H / 2.0);
            systray::click(t, right, sx, sy);
        }
    }

    /// Xử lý chuột của bảng icon ẩn và lịch (cửa sổ Win32 riêng).
    fn poll_flyout(&mut self) {
        for e in shell::take_popup_events(self.cal.hwnd) {
            self.calendar_mouse(e);
        }
        for e in shell::take_popup_events(self.flyout.hwnd) {
            match e {
                shell::PopupEvent::Move(x, y) => self.flyout_mouse(Some((x as f32, y as f32)), None),
                shell::PopupEvent::Leave => self.flyout_mouse(None, None),
                shell::PopupEvent::Up { right } => self.flyout_mouse(Some((-1.0, -1.0)), Some(right)),
                shell::PopupEvent::Outside => {
                    self.close_flyout();
                    self.outside_closed = Some((Instant::now(), 1));
                }
            }
        }
        for e in shell::take_popup_events(self.menu.hwnd) {
            if let Some(id) = self.menu.mouse(e) {
                self.menu_command(id);
            }
        }
    }

    fn toggle_flyout(&mut self) {
        if self.flyout.open {
            self.close_flyout();
        } else {
            self.open_flyout();
        }
    }

    fn open_flyout(&mut self) {
        let k = self.k;
        let n = self.tray_hidden.len();
        let cols = n.clamp(1, 5);
        let rows = n.div_ceil(cols).max(1);
        let (lw, lh) = if n == 0 { (230.0, 56.0 + FLY_TOP) } else { (cols as f32 * FLY_CELL + 20.0, rows as f32 * FLY_CELL + 20.0 + FLY_TOP) };
        let (w, h) = ((lw * k).ceil() as i32, (lh * k).ceil() as i32);
        let (cx, top) = self.screen_pt(self.chevron_x, TOP - 6.0);
        let _ = self.k;
        let x = (cx - w / 2).clamp(self.mon.left + 4, self.mon.right - w - 4);
        let y = top - h;
        let f = &mut self.flyout;
        f.x = x;
        f.y = y;
        f.w = w;
        f.h = h;
        f.cols = cols;
        f.layered = shell::Layered::new(w, h);
        if let Some(l) = f.layered.as_mut() {
            l.alpha = 0;
            l.dy = (shell::FADE_SLIDE * k) as i32;
        }
        f.fade = shell::Fade::open();
        f.open = true;
        self.dirty = true;
        f.hover = None;
        f.close_at = None;
        f.pending = true;
        shell::move_popup(f.hwnd, x, y, w, h);
        shell::watch_outside(self.flyout.hwnd, true);
        self.menu.close();
        self.close_calendar();
        self.next_frame = Instant::now();
    }

    fn close_flyout(&mut self) {
        if !self.flyout.open {
            return;
        }
        self.flyout.fade = shell::Fade::close();
        shell::watch_outside(self.flyout.hwnd, false);
        self.dirty = true;
        self.next_frame = Instant::now();
        self.flyout.open = false;
        self.flyout.hover = None;
        shell::show(self.flyout.hwnd, false);
    }

    fn draw_flyout(&mut self) {
        let k = self.k;
        let f = &self.flyout;
        if f.w <= 0 || f.h <= 0 {
            return;
        }
        let lk = look(self.kind);
        let (lw, lh) = (f.w as f32 / k, f.h as f32 / k);
        let mut cv = Canvas::new(f.w as u32, f.h as u32, Transform::from_scale(k, k));
        let top = FLY_TOP;
        if let Some(p) = round_rect_path(0.5, top + 0.5, lw - 1.0, lh - top - 1.0, 14.0) {
            cv.fill(&p, solid(ca(16, 18, 26, 1.0)));
            let (r, g, b, a) = lk.bg;
            cv.fill(&p, solid(ca(r, g, b, a)));
            let (r, g, b, a) = lk.border;
            cv.stroke(&p, solid(ca(r, g, b, a)), 1.0);
        }
        if self.kind == Kind::Noel {
            let snow = c(0xf6faff);
            cv.round_rect(12.0, top - 6.0, lw - 24.0, 10.0, 5.0, solid(snow));
            cv.round_rect(12.0 + (lw - 24.0) * 0.3, top, 6.0, 7.0, 3.0, solid(snow));
        }
        let mut hits = Vec::new();
        for (i, (_, pm)) in self.tray_hidden.iter().enumerate() {
            let (col, row) = (i % f.cols, i / f.cols);
            let (cx, cy) = (10.0 + col as f32 * FLY_CELL, top + 10.0 + row as f32 * FLY_CELL);
            if f.hover == Some(i) {
                cv.round_rect(cx + 2.0, cy + 2.0, FLY_CELL - 4.0, FLY_CELL - 4.0, 9.0, solid(ca(255, 255, 255, 0.14)));
            }
            // cùng cỡ với icon trên thanh (19 đơn vị)
            let size = (19.0 * k).round();
            let ts = Transform::from_translate(((cx + FLY_CELL / 2.0) * k - size / 2.0).round(), ((cy + FLY_CELL / 2.0) * k - size / 2.0).round()).pre_scale(size / pm.width() as f32, size / pm.width() as f32);
            cv.pm.draw_pixmap(0, 0, pm.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, ts, None);
            hits.push((Rect::from_xywh(cx, cy, FLY_CELL, FLY_CELL).unwrap(), i));
        }
        let mut pm = cv.pm;
        if let Some(fonts) = &self.fonts {
            let label = match f.hover.and_then(|i| self.tray_hidden.get(i)) {
                Some((t, _)) if !t.tip.is_empty() => Some(truncate(&fonts.regular, 12.0, &t.tip, lw - 24.0)),
                _ if self.tray_hidden.is_empty() => Some("Không có icon ẩn".to_string()),
                _ => None,
            };
            if let Some(label) = label {
                let tw = widget::measure(&fonts.regular, 12.0 * k, &label) / k;
                if self.tray_hidden.is_empty() {
                    widget::text(&mut pm, &fonts.regular, 12.5 * k, ((lw - tw) / 2.0) * k, (top + 30.0) * k, lk.fg, 0.8, &label);
                } else {
                    let _ = tw;
                }
            }
        }
        self.flyout.hits = hits;
        let (x, y) = (self.flyout.x, self.flyout.y);
        let hwnd = self.flyout.hwnd;
        if let Some(l) = &mut self.flyout.layered {
            l.present(hwnd, x, y, pm.data());
        }
    }

    /// Chuột trong bảng icon ẩn: `pos` = toạ độ pixel (None = chuột rời bảng; (-1,-1) = giữ nguyên),
    /// `click` = Some(phải?) khi nhả nút chuột.
    fn flyout_mouse(&mut self, pos: Option<(f32, f32)>, click: Option<bool>) {
        let k = self.k;
        let ev = match (pos, click) {
            (_, Some(right)) => FlyEv::Up(right),
            (Some((x, y)), None) => FlyEv::Move(x, y),
            (None, None) => FlyEv::Leave,
        };
        match ev {
            FlyEv::Move(px, py) => {
                let p = (px / k, py / k);
                let h = self.flyout.hits.iter().find(|(r, _)| p.0 >= r.left() && p.0 <= r.right() && p.1 >= r.top() && p.1 <= r.bottom()).map(|(_, i)| *i);
                self.flyout.close_at = None;
                if h != self.flyout.hover {
                    self.flyout.hover = h;
                    self.draw_flyout();
                }
            }
            FlyEv::Leave => {
                self.flyout.close_at = Some(Instant::now() + Duration::from_millis(1200));
                if self.flyout.hover.take().is_some() {
                    self.draw_flyout();
                }
            }
            FlyEv::Up(right) => {
                if let Some(i) = self.flyout.hover {
                    // đóng bảng trước, rồi mới gửi cú bấm: menu của app không bị bảng che hay giành mất
                    let target = self.tray_hidden.get(i).map(|(t, _)| t.clone());
                    let (col, row) = (i % self.flyout.cols, i / self.flyout.cols);
                    let sx = self.flyout.x + ((10.0 + (col as f32 + 0.5) * FLY_CELL) * k) as i32;
                    let sy = self.flyout.y + ((FLY_TOP + 10.0 + (row as f32 + 0.5) * FLY_CELL) * k) as i32;
                    self.close_flyout();
                    if let Some(t) = target {
                        systray::click(&t, right, sx, sy);
                    }
                }
            }
        }
    }

    fn toggle_calendar(&mut self) {
        if self.cal.open {
            self.close_calendar();
        } else {
            self.open_calendar();
        }
    }

    fn open_calendar(&mut self) {
        let k = self.k / SCALE;
        let (w, h) = ((CAL_W * k).ceil() as i32, (CAL_H * k).ceil() as i32);
        let (cx, top) = self.screen_pt(self.clock_x, TOP - 6.0);
        let x = (cx - w / 2).clamp(self.mon.left + 4, self.mon.right - w - 4);
        let y = top - h;
        let c = &mut self.cal;
        c.x = x;
        c.y = y;
        c.w = w;
        c.h = h;
        c.month_off = 0;
        c.hover = None;
        c.open = true;
        c.close_at = None;
        c.layered = shell::Layered::new(w, h);
        if let Some(l) = c.layered.as_mut() {
            l.alpha = 0;
            l.dy = (shell::FADE_SLIDE * k) as i32;
        }
        c.fade = shell::Fade::open();
        shell::move_popup(c.hwnd, x, y, w, h);
        self.menu.close();
        self.close_flyout();
        self.draw_calendar();
        shell::show(self.cal.hwnd, true);
        shell::watch_outside(self.cal.hwnd, true);
    }

    fn close_calendar(&mut self) {
        if !self.cal.open {
            return;
        }
        self.cal.open = false;
        shell::watch_outside(self.cal.hwnd, false);
        self.cal.fade = shell::Fade::close();
    }

    /// Vùng bấm của lịch: -1 = tháng trước, 1 = tháng sau, 0 = về hôm nay.
    fn calendar_hit(&self, x: f32, y: f32) -> Option<i32> {
        let top = FLY_TOP;
        if y >= top + 84.0 && y <= top + 112.0 {
            if x >= CAL_W - 84.0 && x <= CAL_W - 52.0 {
                return Some(-1);
            }
            if x >= CAL_W - 48.0 && x <= CAL_W - 16.0 {
                return Some(1);
            }
            if x >= 16.0 && x <= CAL_W - 96.0 {
                return Some(0);
            }
        }
        None
    }

    fn calendar_mouse(&mut self, e: shell::PopupEvent) {
        let k = self.k / SCALE;
        match e {
            shell::PopupEvent::Move(x, y) => {
                self.cal.close_at = None;
                let h = self.calendar_hit(x as f32 / k, y as f32 / k);
                if h != self.cal.hover {
                    self.cal.hover = h;
                    self.draw_calendar();
                }
            }
            shell::PopupEvent::Leave => {
                self.cal.close_at = None;
                if self.cal.hover.take().is_some() {
                    self.draw_calendar();
                }
            }
            shell::PopupEvent::Up { .. } => {
                match self.cal.hover {
                    Some(0) => self.cal.month_off = 0,
                    Some(d) => self.cal.month_off += d,
                    None => {}
                }
                self.draw_calendar();
            }
            shell::PopupEvent::Outside => {
                self.close_calendar();
                self.outside_closed = Some((Instant::now(), 2));
            }
        }
    }

    /// Cửa sổ nổi `which` (1 = bảng ^, 2 = lịch) vừa bị đóng do bấm ra ngoài – chính là cú bấm này.
    fn just_closed(&mut self, which: u8) -> bool {
        matches!(self.outside_closed.take(), Some((t, w)) if w == which && t.elapsed() < Duration::from_millis(600))
    }

    fn draw_calendar(&mut self) {
        let Some(fonts) = &self.fonts else { return };
        let cal = &self.cal;
        if cal.w <= 0 || cal.h <= 0 {
            return;
        }
        let k = self.k / SCALE;
        let lk = look(self.kind);
        let acc = ca(lk.acc[0], lk.acc[1], lk.acc[2], 1.0);
        let (lw, lh) = (cal.w as f32 / k, cal.h as f32 / k);
        let mut cv = Canvas::new(cal.w as u32, cal.h as u32, Transform::from_scale(k, k));
        let top = FLY_TOP;
        if let Some(p) = round_rect_path(0.5, top + 0.5, lw - 1.0, lh - top - 1.0, 16.0) {
            cv.fill(&p, solid(ca(16, 18, 26, 1.0)));
            let (r, g, b, a) = lk.bg;
            cv.fill(&p, solid(ca(r, g, b, a)));
            let (r, g, b, a) = lk.border;
            cv.stroke(&p, solid(ca(r, g, b, a)), 1.0);
        }
        if self.kind == Kind::Noel {
            let snow = c(0xf6faff);
            cv.round_rect(14.0, top - 6.0, lw - 28.0, 10.0, 5.0, solid(snow));
            cv.round_rect(14.0 + (lw - 28.0) * 0.7, top, 6.0, 8.0, 3.0, solid(snow));
        }
        let now = local_time();
        // tháng đang xem
        let total = now.year * 12 + now.month as i32 - 1 + cal.month_off;
        let (vy, vm) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
        let first = widget::days(vy, vm, 1);
        let next = if vm == 12 { widget::days(vy + 1, 1, 1) } else { widget::days(vy, vm + 1, 1) };
        let ndays = (next - first) as u32;
        let lead = ((first + 4).rem_euclid(7) + 6) % 7; // thứ Hai đầu tuần
        // ô hôm nay, nút tháng
        let grid_top = top + 150.0;
        let (cw, ch) = ((lw - 24.0) / 7.0, 32.0);
        let mut texts: Vec<(bool, f32, f32, f32, [u8; 3], f32, String)> = Vec::new();
        let fg = lk.fg;
        let (ld, lm, ly, leap) = crate::lunar::solar_to_lunar(now.day, now.month, now.year);
        const WD: [&str; 7] = ["Chủ Nhật", "Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"];
        texts.push((true, 17.0, 18.0, top + 34.0, fg, 1.0, format!("{}, {} tháng {}", WD[(now.weekday % 7) as usize], now.day, now.month)));
        texts.push((false, 12.5, 18.0, top + 56.0, lk.acc, 1.0, format!("Âm lịch: {} tháng {}{} năm {}", ld, lm, if leap { " (nhuận)" } else { "" }, crate::lunar::can_chi(ly))));
        if let Some((kk, d)) = crate::config::next_event(&now) {
            let t = if d == 0 { format!("Hôm nay: {}", kk.name()) } else { format!("{}: còn {d} ngày", kk.name()) };
            texts.push((false, 12.0, 18.0, top + 74.0, fg, 0.7, t));
        }
        // tiêu đề tháng + nút ‹ ›
        if cal.hover == Some(0) && cal.month_off != 0 {
            cv.round_rect(12.0, top + 84.0, lw - 108.0, 28.0, 8.0, solid(ca(255, 255, 255, 0.10)));
        }
        texts.push((true, 15.0, 18.0, top + 104.0, fg, 1.0, format!("Tháng {vm}, {vy}")));
        for (d, x) in [(-1, lw - 84.0), (1, lw - 48.0)] {
            if cal.hover == Some(d) {
                cv.round_rect(x, top + 84.0, 32.0, 28.0, 8.0, solid(ca(255, 255, 255, 0.14)));
            }
            let mut pb = PathBuilder::new();
            let (mx, my) = (x + 16.0, top + 98.0);
            if d < 0 {
                pb.move_to(mx + 3.0, my - 6.0);
                pb.line_to(mx - 3.0, my);
                pb.line_to(mx + 3.0, my + 6.0);
            } else {
                pb.move_to(mx - 3.0, my - 6.0);
                pb.line_to(mx + 3.0, my);
                pb.line_to(mx - 3.0, my + 6.0);
            }
            if let Some(p) = pb.finish() {
                cv.stroke(&p, solid(ca(fg[0], fg[1], fg[2], 1.0)), 1.8);
            }
        }
        for (i, n) in ["T2", "T3", "T4", "T5", "T6", "T7", "CN"].iter().enumerate() {
            let tw = widget::measure(&fonts.regular, 12.0 * k, n) / k;
            texts.push((false, 12.0, 12.0 + cw * (i as f32 + 0.5) - tw / 2.0, grid_top - 12.0, fg, 0.55, n.to_string()));
        }
        for d in 1..=ndays {
            let idx = lead as u32 + d - 1;
            let (col, row) = ((idx % 7) as f32, (idx / 7) as f32);
            let (cx0, cy0) = (12.0 + cw * col, grid_top + ch * row);
            let today = vy == now.year && vm == now.month && d == now.day;
            if today {
                cv.circle(cx0 + cw / 2.0, cy0 + ch / 2.0 - 1.0, 15.0, solid(acc));
            }
            let s = d.to_string();
            let tw = widget::measure(&fonts.semibold, 13.0 * k, &s) / k;
            let color = if today { [16, 18, 26] } else { fg };
            texts.push((today, 13.0, cx0 + cw / 2.0 - tw / 2.0, cy0 + 17.0, color, if col == 6.0 && !today { 0.75 } else { 1.0 }, s));
            // ngày âm nhỏ bên dưới (mùng 1 thì ghi cả tháng)
            let (ldd, lmm, _, _) = crate::lunar::solar_to_lunar(d, vm, vy);
            let ls = if ldd == 1 { format!("{ldd}/{lmm}") } else { ldd.to_string() };
            let lw2 = widget::measure(&fonts.regular, 9.0 * k, &ls) / k;
            let lc = if today { [16, 18, 26] } else if ldd == 1 || ldd == 15 { lk.acc } else { fg };
            texts.push((false, 9.0, cx0 + cw / 2.0 - lw2 / 2.0, cy0 + 28.0, lc, if today { 0.9 } else { 0.55 }, ls));
        }
        let mut pm = cv.pm;
        for (b, px, x, bl, col, a, s) in &texts {
            let f = if *b { &fonts.semibold } else { &fonts.regular };
            widget::text(&mut pm, f, px * k, x * k, bl * k, *col, *a, s);
        }
        let (x, y, hwnd) = (self.cal.x, self.cal.y, self.cal.hwnd);
        if let Some(l) = &mut self.cal.layered {
            l.present(hwnd, x, y, pm.data());
        }
    }

    fn hit_at(&self, p: (f32, f32)) -> Option<Hit> {
        self.hits.iter().rev().find(|(r, _)| p.0 >= r.left() && p.0 <= r.right() && p.1 >= r.top() && p.1 <= r.bottom()).map(|(_, h)| *h)
    }

    fn island(&self, cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, lk: &Look) {
        for i in 0..3 {
            let s = 2.0 + i as f32 * 5.0;
            if let Some(p) = round_rect_path(x - s * 0.5, y + 6.0 + s * 0.3, w + s, h + s * 0.3, 18.0 + s * 0.5) {
                cv.fill(&p, solid(ca(0, 0, 0, 0.060)));
            }
        }
        let Some(path) = round_rect_path(x, y, w, h, 18.0) else { return };
        match &self.backdrop {
            Some(bd) => {
                let band = BAND.min((self.mon.bottom - self.mon.top) as u32) as f32;
                let ts = Transform::from_row(
                    self.wpx as f32 / (bd.width() as f32 * self.k), 0.0,
                    0.0, band / (bd.height() as f32 * self.k),
                    0.0, (self.hpx as f32 - band) / self.k,
                );
                cv.fill(&path, Pattern::new(bd.as_ref(), SpreadMode::Pad, FilterQuality::Bilinear, 1.0, ts));
            }
            None => cv.fill(&path, solid(ca(18, 20, 28, 0.62))),
        }
        let (r, g, b, a) = lk.bg;
        let tint = crate::paint::lin(x, y, x, y + h, &[
            (0.0, ca(r, g, b, (a + 0.12).min(1.0))),
            (1.0, ca(r / 2, g / 2, b / 2, (a + 0.25).min(1.0))),
        ]);
        cv.fill(&path, tint);
        let (r, g, b, a) = lk.border;
        cv.stroke(&path, solid(ca(r, g, b, a)), 1.0);
        let [r, g, b] = lk.acc;
        if w > 46.0 {
            let left = x + 18.0;
            let right = x + w - 18.0;
            let rim = crate::paint::lin(left, y, right, y, &[
                (0.0, ca(r, g, b, 0.04)),
                (0.50, ca(r, g, b, 0.56)),
                (1.0, ca(r, g, b, 0.04)),
            ]);
            cv.line(left, y + 1.3, right, y + 1.3, rim, 1.3);
        }
    }

    fn hover_bg(&self, cv: &mut Canvas, hit: Hit, x: f32, y: f32, w: f32, h: f32) {
        if self.hover == Some(hit) {
            cv.round_rect(x, y, w, h, 9.0, solid(ca(255, 255, 255, 0.12)));
        }
    }

    fn draw(&mut self) {
        let Some(fonts) = &self.fonts else { return };
        let k = self.k;
        let mut anim = Anim::default();
        let lk = look(self.kind);
        let fg = ca(lk.fg[0], lk.fg[1], lk.fg[2], 1.0);
        let (acc_rgb, fg_rgb) = (lk.acc, lk.fg);
        let acc = ca(acc_rgb[0], acc_rgb[1], acc_rgb[2], 1.0);
        let now = local_time();
        let lw = self.wpx as f32 / k;
        let (y, h) = (TOP, BAR_H);
        let mut hits: Vec<(Rect, Hit)> = Vec::new();
        let mut cv = Canvas::new(self.wpx as u32, self.hpx as u32, Transform::from_scale(k, k));
        let mut texts: Vec<(bool, f32, f32, f32, [u8; 3], f32, String)> = Vec::new(); // (semibold, px, x, baseline, màu, alpha, chữ)

        // Màn hình phụ: vẽ hai đảo bên vào chỗ bỏ đi (chỉ giữ đảo giữa)
        let side_mark = (texts.len(), hits.len());
        let saved = (!self.primary).then(|| std::mem::replace(&mut cv, Canvas::new(1, 1, Transform::identity())));

        // ---------- đảo trái: nhạc đang phát hoặc lời chúc
        let has_media = self.minfo.has && !self.minfo.title.is_empty();
        let (title, sub) = if has_media {
            let state = if self.minfo.playing { "Đang phát" } else { "Tạm dừng" };
            let sub = if self.minfo.artist.is_empty() { state.to_string() } else { format!("{state} · {}", self.minfo.artist) };
            (self.minfo.title.clone(), sub)
        } else {
            (widget::greeting(self.kind).to_string(), widget::chip_text(self.kind, &now))
        };
        let title = truncate(&fonts.semibold, 14.0, &title, 250.0);
        let sub = truncate(&fonts.regular, 12.0, &sub, 250.0);
        let tw = widget::measure(&fonts.semibold, 14.0 * k, &title).max(widget::measure(&fonts.regular, 12.0 * k, &sub)) / k;
        let left_w = 16.0 + 24.0 + 10.0 + tw + 16.0;
        let lx = SIDE;
        self.island(&mut cv, lx, y, left_w, h, &lk);
        if has_media {
            self.hover_bg(&mut cv, Hit::Media, lx + 6.0, y + 6.0, left_w - 12.0, h - 12.0);
            hits.push((Rect::from_xywh(lx, y, left_w, h).unwrap(), Hit::Media));
            if self.minfo.playing {
                anim.eq = Some((lx + 16.0, y + 39.0, acc_rgb));
            } else {
                for i in 0..4 {
                    cv.round_rect(lx + 16.0 + i as f32 * 7.0, y + 34.0, 4.0, 5.0, 2.0, solid(acc));
                }
            }
        } else {
            self.season_glyph(&mut cv, lx + 16.0, y + 17.0, acc);
        }
        texts.push((true, 14.0, lx + 50.0, y + 26.0, fg_rgb, 1.0, title));
        texts.push((false, 12.0, lx + 50.0, y + 44.0, fg_rgb, 0.75, sub));

        if let Some(c) = saved {
            cv = c;
            texts.truncate(side_mark.0);
            hits.truncate(side_mark.1);
            anim.eq = None;
        }

        // ---------- đảo giữa: Start + app
        // Hiện hết các ô vừa chỗ trống giữa hai đảo bên (như taskbar gốc)
        // màn phụ chỉ có đảo giữa nên dùng gần hết bề rộng cho các ô
        let reserved = if self.primary { 700.0 } else { 120.0 };
        let n = self.apps.len().min(((lw - reserved) / 50.0).max(8.0) as usize);
        let target_w = 12.0 + 42.0 + if n > 0 { 9.0 + n as f32 * 42.0 + (n as f32 - 1.0) * 8.0 } else { 0.0 } + 12.0;
        // Hiệu ứng như taskbar gốc: đảo co giãn, ô trượt tới chỗ mới, ô mới lướt lên, ô đóng lướt xuống
        let t_now = Instant::now();
        let ease = |p: f32| 1.0 - (1.0 - p.clamp(0.0, 1.0)).powi(3);
        let slide_at = |a: &(f32, f32, Instant)| {
            let p = a.2.elapsed().as_secs_f32() * 1000.0 / TILE_SLIDE_MS;
            (a.0 + (a.1 - a.0) * ease(p), p < 1.0)
        };
        let mut busy = false;
        if self.island_w.1 == 0.0 {
            self.island_w = (target_w, target_w, t_now);
        } else if (self.island_w.1 - target_w).abs() > 0.1 {
            let (cur, _) = slide_at(&self.island_w);
            self.island_w = (cur, target_w, t_now);
        }
        let (center_w, b) = slide_at(&self.island_w);
        busy |= b;
        let cx = (lw - center_w) / 2.0;
        // vị trí đích của các ô theo bề rộng cuối cùng
        let t_tx = (lw - target_w) / 2.0 + 12.0 + 42.0 + 9.0;
        self.tiles_x0 = t_tx;
        let dragging = self.drag.as_ref().filter(|d| d.active).map(|d| (d.key.clone(), d.grab));
        let first_draw = self.tile_slide.is_empty();
        self.island(&mut cv, cx, y, center_w, h, &lk);
        let sx = cx + 12.0;
        self.hover_bg(&mut cv, Hit::Start, sx, y + 8.0, 42.0, 42.0);
        hits.push((Rect::from_xywh(sx, y + 8.0, 42.0, 42.0).unwrap(), Hit::Start));
        for (ox, oy, a) in [(0.0, 0.0, 1.0), (10.0, 0.0, 0.8), (0.0, 10.0, 0.8), (10.0, 10.0, 0.6)] {
            cv.round_rect(sx + 12.0 + ox, y + 17.0 + oy, 8.0, 8.0, 2.2, solid(ca(fg_rgb[0], fg_rgb[1], fg_rgb[2], a)));
        }
        if n > 0 {
            let tx = sx + 42.0;
            cv.rect(tx + 4.0, y + 15.0, 1.0, 28.0, solid(ca(lk.border.0, lk.border.1, lk.border.2, lk.border.3)));
        }
        let mut tile_x = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for (i, app) in self.apps.iter().take(n).enumerate() {
            let target = t_tx + i as f32 * 50.0;
            seen.push(app.key.as_str());
            if !self.tile_slide.contains_key(&app.key) {
                // Ô mới: app vừa mở (chưa ghim) thì lướt từ dưới lên
                if !first_draw && app.lnk.is_none() {
                    self.tile_born.insert(app.key.clone(), t_now);
                }
                self.tile_slide.insert(app.key.clone(), (target, target, t_now));
            }
            let e = self.tile_slide.get_mut(&app.key).unwrap();
            if (e.1 - target).abs() > 0.1 {
                let (cur, _) = slide_at(e);
                *e = (cur, target, t_now);
            }
            // Ô đang kéo đi theo con trỏ (trong phạm vi các ô); các ô khác trượt nhường chỗ
            if let Some((_, grab)) = dragging.as_ref().filter(|(key, _)| *key == app.key) {
                let dx = (self.cursor.0 - grab).clamp(t_tx, t_tx + (n.max(1) - 1) as f32 * 50.0);
                *e = (dx, dx, t_now - Duration::from_secs(1));
            }
            let (x, b) = slide_at(e);
            busy |= b;
            if let Some(ic) = &app.icon {
                if !self.tile_icons.contains_key(&app.key) {
                    self.tile_icons.insert(app.key.clone(), ic.clone());
                }
            }
            let (rise, fade) = match self.tile_born.get(&app.key) {
                Some(t0) => {
                    let p = t0.elapsed().as_secs_f32() * 1000.0 / TILE_APPEAR_MS;
                    if p >= 1.0 {
                        self.tile_born.remove(&app.key);
                        (0.0, 1.0)
                    } else {
                        busy = true;
                        ((1.0 - ease(p)) * 26.0, ease(p))
                    }
                }
                None => (0.0, 1.0),
            };
            let ty = y + 8.0 + rise;
            tile_x.push((x, app.hwnd));
            // Chỉ icon của app, không có ô vuông nền; rê chuột hoặc đang mở thì có nền mờ nhẹ.
            let active = app.wins.contains(&self.fg);
            // Nền ô: rê chuột / đang chọn / đang mở nhiều cửa sổ
            let multi = app.wins.len() > 1;
            // Như taskbar gốc: chỉ ô đang chọn (hoặc đang rê chuột) mới có nền; đang mở mà không chọn chỉ có chấm
            let plate = if self.hover == Some(Hit::App(app.hwnd)) {
                0.16
            } else if active {
                0.12
            } else {
                0.0
            };
            if plate > 0.0 {
                cv.round_rect(x, ty, 42.0, 42.0, 9.0, solid(ca(255, 255, 255, plate)));
            }
            // Nhiều cửa sổ: mép một ô nữa xếp chồng bên phải (như taskbar gốc), có khe hở để dễ nhìn
            if multi && plate > 0.0 {
                cv.round_rect(x + 43.5, ty + 5.0, 3.0, 32.0, 1.5, solid(ca(255, 255, 255, (plate + 0.08).min(0.3))));
            }
            if let Some(ic) = &app.icon {
                // Icon đã lấy đúng kích thước pixel: đặt khớp lưới điểm ảnh, không co giãn → nét.
                let size_px = ic.width() as f32;
                let px0 = ((x + 21.0) * k - size_px / 2.0).round() as i32;
                let py0 = ((ty + 21.0) * k - size_px / 2.0).round() as i32;
                cv.pm.draw_pixmap(px0, py0, ic.as_ref(), &PixmapPaint { quality: FilterQuality::Nearest, opacity: fade, ..Default::default() }, Transform::identity(), None);
            } else {
                cv.round_rect(x + 9.0, ty + 9.0, 24.0, 24.0, 6.0, solid(ca(fg_rgb[0], fg_rgb[1], fg_rgb[2], 0.3 * fade)));
            }
            // Vạch dưới icon như taskbar gốc: chỉ app đang mở mới có (đang chọn thì dài và có màu).
            if active {
                cv.round_rect(x + 14.0, ty + 46.0 - rise, 14.0, 3.0, 1.5, solid(ca(acc_rgb[0], acc_rgb[1], acc_rgb[2], fade)));
            } else if app.hwnd > 0 {
                cv.round_rect(x + 18.5, ty + 46.0 - rise, 5.0, 3.0, 1.5, solid(ca(fg_rgb[0], fg_rgb[1], fg_rgb[2], 0.45 * fade)));
            }
            hits.push((Rect::from_xywh(x, y + 8.0, 42.0, 42.0).unwrap(), Hit::App(app.hwnd)));
        }
        // Ô vừa biến mất (app vừa đóng): lướt xuống và mờ đi tại chỗ cũ
        let gone: Vec<String> = self.tile_slide.keys().filter(|k| !seen.contains(&k.as_str())).cloned().collect();
        for key in gone {
            if let Some(e) = self.tile_slide.remove(&key) {
                let (x, _) = slide_at(&e);
                if !first_draw {
                    self.tile_dying.push((key.clone(), x, self.tile_icons.get(&key).cloned(), t_now));
                }
            }
            self.tile_icons.remove(&key);
            self.tile_born.remove(&key);
        }
        self.tile_dying.retain(|d| d.3.elapsed().as_secs_f32() * 1000.0 < TILE_APPEAR_MS);
        for (_, x, ic, t0) in &self.tile_dying {
            let p = ease(t0.elapsed().as_secs_f32() * 1000.0 / TILE_APPEAR_MS);
            busy = true;
            if let Some(ic) = ic {
                let size_px = ic.width() as f32;
                let px0 = ((x + 21.0) * k - size_px / 2.0).round() as i32;
                let py0 = ((y + 8.0 + 21.0 + p * 26.0) * k - size_px / 2.0).round() as i32;
                cv.pm.draw_pixmap(px0, py0, ic.as_ref(), &PixmapPaint { quality: FilterQuality::Nearest, opacity: 1.0 - p, ..Default::default() }, Transform::identity(), None);
            }
        }
        self.tiles_animating = busy;

        let side_mark = (texts.len(), hits.len());
        let saved = (!self.primary).then(|| std::mem::replace(&mut cv, Canvas::new(1, 1, Transform::identity())));

        // ---------- đảo phải: cài đặt, khay ẩn, hệ thống, đồng hồ
        let time = format!("{:02}:{:02}", now.hour, now.minute);
        let date = format!("{:02}/{:02}/{}", now.day, now.month, now.year);
        let clock_w = (widget::measure(&fonts.regular, 14.0 * k, &time).max(widget::measure(&fonts.regular, 11.5 * k, &date))) / k;
        let net_icon = match self.net {
            shell::Net::Wifi => Icon::Wifi,
            shell::Net::Ethernet => Icon::Ethernet,
            shell::Net::Offline => Icon::Offline,
        };
        let mut sys: Vec<Icon> = vec![net_icon, Icon::Volume(self.vol.0, self.vol.1)];
        if let Some(p) = self.battery {
            sys.push(Icon::Battery(p));
        }
        let lixi = self.kind == Kind::Tet;
        let ntray = self.tray_pinned.len() as f32;
        let icons_w = 19.0 + 12.0 + ntray * 31.0 + if lixi { 16.0 + 12.0 } else { 0.0 } + sys.len() as f32 * 19.0 + (sys.len() as f32 - 1.0) * 12.0;
        let right_w = 16.0 + icons_w + 25.0 + clock_w + 16.0;
        let rx = lw - SIDE - right_w;
        self.island(&mut cv, rx, y, right_w, h, &lk);
        let mut ix = rx + 16.0;
        let iy = y + (h - 19.0) / 2.0;
        self.hover_bg(&mut cv, Hit::Peek, ix - 6.0, y + 10.0, 31.0, 38.0);
        {
            // mũi tên xoay nửa vòng (hướng lên ↔ hướng xuống) trong 200 ms, chậm dần ở cuối
            let target = if self.flyout.open { std::f32::consts::PI } else { 0.0 };
            if (self.chev.1 - target).abs() > 1e-3 {
                let p = (self.chev.2.elapsed().as_secs_f32() / 0.2).clamp(0.0, 1.0);
                let cur = self.chev.0 + (self.chev.1 - self.chev.0) * (1.0 - (1.0 - p).powi(3));
                self.chev = (cur, target, Instant::now());
            }
            let p = (self.chev.2.elapsed().as_secs_f32() / 0.2).clamp(0.0, 1.0);
            let ang = self.chev.0 + (self.chev.1 - self.chev.0) * (1.0 - (1.0 - p).powi(3));
            if p < 1.0 {
                self.tiles_animating = true;
            }
            let base = cv.base;
            cv.base = base.pre_concat(Transform::from_rotate_at(ang.to_degrees(), ix + 9.5, iy + 9.5));
            icon(&mut cv, Icon::Chevron, ix, iy, 19.0, fg);
            cv.base = base;
        }
        hits.push((Rect::from_xywh(ix - 6.0, y + 8.0, 31.0, 42.0).unwrap(), Hit::Peek));
        let chevron_x = ix + 9.5;
        ix += 31.0;
        // Icon ghim ở khay (ảnh thật do Windows lưu), cùng thứ tự với khay gốc
        let mut tray_x = Vec::new();
        for (i, (_, pm)) in self.tray_pinned.iter().enumerate() {
            self.hover_bg(&mut cv, Hit::Tray(i), ix - 6.0, y + 10.0, 31.0, 38.0);
            let px = (19.0 * k).round();
            let sc = px / pm.width().max(1) as f32;
            let x0 = (ix * k).round();
            let y0 = (iy * k).round();
            cv.pm.draw_pixmap(0, 0, pm.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, Transform::from_translate(x0, y0).pre_scale(sc, sc), None);
            hits.push((Rect::from_xywh(ix - 6.0, y + 8.0, 31.0, 42.0).unwrap(), Hit::Tray(i)));
            tray_x.push(ix + 9.5);
            ix += 31.0;
        }
        if lixi {
            let (lx2, ly2) = (ix, y + 19.0);
            cv.round_rect(lx2 - 1.0, ly2 - 1.0, 18.0, 22.0, 4.0, solid(c(0xffcf6b)));
            cv.round_rect(lx2, ly2, 16.0, 20.0, 3.0, solid(c(0xd8282f)));
            cv.circle(lx2 + 8.0, ly2 + 10.0, 3.0, solid(c(0xffcf6b)));
            ix += 28.0;
        }
        let mut vol_x = 0.0;
        for ic in sys {
            let hit = if matches!(ic, Icon::Volume(..)) { Hit::Volume } else { Hit::Net };
            if hit == Hit::Volume {
                vol_x = ix + 9.5;
            }
            self.hover_bg(&mut cv, hit, ix - 6.0, y + 10.0, 31.0, 38.0);
            hits.push((Rect::from_xywh(ix - 6.0, y + 8.0, 31.0, 42.0).unwrap(), hit));
            icon(&mut cv, ic, ix, iy, 19.0, fg);
            ix += 31.0;
        }
        ix -= 12.0;
        cv.rect(ix + 12.0, y + 15.0, 1.0, 28.0, solid(ca(lk.border.0, lk.border.1, lk.border.2, lk.border.3)));
        let clock_x = ix + 25.0;
        self.hover_bg(&mut cv, Hit::Clock, clock_x - 6.0, y + 10.0, clock_w + 12.0, 38.0);
        hits.push((Rect::from_xywh(clock_x - 6.0, y + 8.0, clock_w + 12.0, 42.0).unwrap(), Hit::Clock));
        let tw1 = widget::measure(&fonts.regular, 14.0 * k, &time) / k;
        let tw2 = widget::measure(&fonts.regular, 11.5 * k, &date) / k;
        texts.push((false, 14.0, clock_x + clock_w - tw1, y + 27.0, fg_rgb, 1.0, time));
        texts.push((false, 11.5, clock_x + clock_w - tw2, y + 43.0, fg_rgb, 0.75, date));

        if let Some(c) = saved {
            cv = c;
            texts.truncate(side_mark.0);
            hits.truncate(side_mark.1);
        }

        // ---------- trang trí theo mùa
        // (vẽ tĩnh vào ảnh đệm: taskbar không cần hiệu ứng chuyển động)
        if matches!(self.kind, Kind::Noel | Kind::Winter) && self.primary {
            self.snowcap(&mut cv, lx, y, left_w);
            self.snowcap(&mut cv, rx, y, right_w);
        }
        self.ornament(&mut cv, cx, y, center_w);

        // ---------- chữ
        let mut pm = cv.pm;
        for (bold, px, x, bl, col, a, s) in &texts {
            let f = if *bold { &fonts.semibold } else { &fonts.regular };
            widget::text(&mut pm, f, px * k, x * k, bl * k, *col, *a, s);
        }

        self.tile_pos = tile_x.clone();
        // ---------- tên app / icon khay khi rê chuột
        let tip: Option<(f32, String)> = match self.hover {
            // App đang mở: có ảnh xem trước thay cho chữ
            Some(Hit::App(hw)) if hw < 0 => match (self.apps.iter().find(|a| a.hwnd == hw), tile_x.iter().find(|(_, h)| *h == hw)) {
                (Some(app), Some((x, _))) => Some((x + 21.0, if app.wins.len() > 1 { format!("{} · {} cửa sổ", app.title, app.wins.len()) } else { app.title.clone() })),
                _ => None,
            },
            Some(Hit::Tray(i)) => self.tray_pinned.get(i).zip(tray_x.get(i)).map(|((t, _), x)| (*x, t.tip.clone())),
            Some(Hit::Volume) => Some((vol_x, if self.vol.1 { "Đang tắt tiếng · cuộn để chỉnh".to_string() } else { format!("Âm lượng {}% · cuộn để chỉnh", (self.vol.0 * 100.0).round()) })),
            _ => None,
        };
        if let Some((cxp, title)) = tip {
            {
                let label = truncate(&fonts.regular, 12.5, &title, 360.0);
                let w = widget::measure(&fonts.regular, 12.5 * k, &label) / k + 24.0;
                let px0 = (cxp - w / 2.0).clamp(4.0, lw - w - 4.0);
                let mut cv2 = Canvas { pm, base: Transform::from_scale(k, k) };
                cv2.round_rect(px0, y - 36.0, w, 26.0, 8.0, solid(ca(lk.border.0, lk.border.1, lk.border.2, 0.6)));
                cv2.round_rect(px0 + 1.0, y - 35.0, w - 2.0, 24.0, 7.0, solid(ca(15, 17, 22, 0.94)));
                pm = cv2.pm;
                widget::text(&mut pm, &fonts.regular, 12.5 * k, (px0 + 12.0) * k, (y - 19.0) * k, fg_rgb, 1.0, &label);
            }
        }

        self.hits = hits;
        self.clock_x = clock_x + clock_w / 2.0;
        self.tray_pos = tray_x;
        self.chevron_x = chevron_x;
        // Dải 3 px dưới đáy gần như trong suốt nhưng vẫn "bắt" chuột, để taskbar gốc
        // (đang tự ẩn) không trồi lên khi chuột chạm đáy màn hình.
        let (w, h) = (pm.width() as usize, pm.height() as usize);
        let data = pm.data_mut();
        for y in h.saturating_sub(3)..h {
            for x in 0..w {
                let i = (y * w + x) * 4 + 3;
                if data[i] == 0 {
                    data[i] = 1;
                }
            }
        }
        self.anim = anim;
        self.cache = Some(pm);
    }

    fn season_glyph(&self, cv: &mut Canvas, x: f32, y: f32, acc: Color) {
        let (cx, cy) = (x + 12.0, y + 12.0);
        let theme = self.kind.theme();
        let [first, second, third] = theme.dots.map(c);
        let ink = c(0x33253a);
        let shape = |cv: &mut Canvas, points: &[(f32, f32)], color: Color| {
            let mut p = PathBuilder::new();
            p.move_to(cx + points[0].0, cy + points[0].1);
            for &(dx, dy) in &points[1..] {
                p.line_to(cx + dx, cy + dy);
            }
            p.close();
            if let Some(path) = p.finish() {
                cv.fill(&path, solid(color));
            }
        };
        match self.kind {
            Kind::Spring | Kind::Tet => {
                for i in 0..5 {
                    let a = i as f32 * std::f32::consts::TAU / 5.0;
                    cv.ellipse(cx + a.cos() * 5.3, cy + a.sin() * 5.3,
                        5.1, 3.6, a, solid(first));
                }
                cv.circle(cx, cy, 3.0, solid(if self.kind == Kind::Tet { second } else { c(0xf6cf80) }));
            }
            Kind::Summer => {
                cv.circle(cx + 3.5, cy - 4.5, 5.7, solid(first));
                let mut p = PathBuilder::new();
                p.move_to(cx - 10.0, cy + 5.0);
                p.cubic_to(cx - 5.5, cy - 1.0, cx + 1.0, cy + 9.5, cx + 10.0, cy + 2.0);
                if let Some(path) = p.finish() {
                    cv.stroke(&path, solid(acc), 2.8);
                }
            }
            Kind::Autumn => {
                let mut p = PathBuilder::new();
                p.move_to(cx - 7.5, cy + 8.0);
                p.cubic_to(cx - 11.0, cy - 1.0, cx - 2.0, cy - 8.0, cx + 8.8, cy - 8.8);
                p.cubic_to(cx + 10.0, cy + 1.5, cx + 3.0, cy + 9.0, cx - 7.5, cy + 8.0);
                p.close();
                if let Some(path) = p.finish() {
                    cv.fill(&path, solid(first));
                }
                cv.line(cx - 8.8, cy + 9.0, cx + 5.0, cy - 5.2, solid(acc), 1.4);
            }
            Kind::Winter => icon(cv, Icon::Snowflake, x, y, 24.0, acc),
            Kind::Noel => {
                shape(cv, &[(0.0, -10.0), (7.0, 0.0), (4.3, 0.0), (9.0, 7.0),
                    (-9.0, 7.0), (-4.3, 0.0), (-7.0, 0.0)], second);
                cv.circle(cx, cy - 9.0, 2.1, solid(acc));
                for (dx, dy) in [(-3.6, 3.0), (3.4, 3.0), (0.0, -1.0)] {
                    cv.circle(cx + dx, cy + dy, 1.3, solid(third));
                }
            }
            Kind::Moon => {
                if let Some(path) = star_path(cx, cy - 1.0, 9.6, 4.8, 5) {
                    cv.fill(&path, solid(first));
                    cv.stroke(&path, solid(third), 1.2);
                }
                cv.line(cx, cy + 7.6, cx, cy + 11.0, solid(acc), 1.3);
            }
            Kind::NewYear => {
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.0;
                    let (sin, cos) = a.sin_cos();
                    cv.line(cx + cos * 4.0, cy + sin * 4.0, cx + cos * 9.0, cy + sin * 9.0,
                        solid(if i % 2 == 0 { first } else { third }), 1.8);
                }
                cv.circle(cx, cy, 1.8, solid(acc));
            }
            Kind::Valentine => {
                let mut p = PathBuilder::new();
                p.move_to(cx, cy + 8.6);
                p.cubic_to(cx - 16.0, cy - 0.8, cx - 7.0, cy - 13.5, cx, cy - 4.0);
                p.cubic_to(cx + 7.0, cy - 13.5, cx + 16.0, cy - 0.8, cx, cy + 8.6);
                p.close();
                if let Some(path) = p.finish() {
                    cv.fill(&path, solid(first));
                    cv.stroke(&path, solid(acc), 0.8);
                }
            }
            Kind::Women => {
                cv.line(cx - 6.6, cy + 8.6, cx + 6.0, cy - 7.0, solid(third), 1.6);
                for (dx, dy) in [(-3.0, 1.3), (-2.0, -3.2), (1.0, 0.0), (2.8, -5.7), (6.2, -3.9), (6.4, -8.3)] {
                    cv.circle(cx + dx, cy + dy, 2.6, solid(first));
                }
                cv.ellipse(cx - 4.2, cy + 6.7, 4.0, 1.5, -0.58, solid(second));
            }
            Kind::VnWomen => {
                cv.ellipse(cx, cy + 6.3, 10.0, 2.7, 0.0, solid(third));
                for (dx, a) in [(-4.7, -0.76), (4.7, 0.76)] {
                    cv.ellipse(cx + dx, cy + 0.5, 3.5, 6.7, a, solid(first));
                }
                cv.ellipse(cx, cy - 3.0, 3.4, 6.8, 0.0, solid(second));
            }
            Kind::Reunify | Kind::National => {
                let (ox, oy) = if self.kind == Kind::Reunify {
                    cv.line(cx - 8.0, cy - 8.7, cx - 8.0, cy + 9.2, solid(acc), 1.6);
                    shape(cv, &[(-7.1, -8.0), (9.6, -7.4), (7.4, 3.6), (-7.1, 3.1)], first);
                    (cx + 0.7, cy - 2.2)
                } else {
                    cv.circle(cx, cy, 10.1, solid(first));
                    (cx, cy)
                };
                if let Some(path) = star_path(ox, oy, 5.0, 2.1, 5) {
                    cv.fill(&path, solid(second));
                }
            }
            Kind::Children => {
                shape(cv, &[(0.0, -9.7), (8.6, -2.0), (0.0, 6.2), (-8.6, -2.0)], second);
                shape(cv, &[(0.0, -9.7), (8.6, -2.0), (0.0, 6.2)], first);
                cv.line(cx, cy + 6.0, cx - 4.7, cy + 11.0, solid(third), 1.4);
            }
            Kind::Halloween => {
                cv.ellipse(cx, cy + 1.0, 9.8, 7.6, 0.0, solid(first));
                cv.ellipse(cx, cy + 1.0, 5.8, 7.6, 0.0, solid(acc));
                cv.line(cx, cy - 6.0, cx + 2.0, cy - 10.0, solid(c(0x9cad66)), 2.3);
                shape(cv, &[(-5.0, 0.0), (-2.6, -2.9), (-1.2, 0.0)], ink);
                shape(cv, &[(1.2, 0.0), (3.0, -2.9), (5.3, 0.0)], ink);
                cv.line(cx - 3.2, cy + 4.0, cx + 3.2, cy + 4.0, solid(ink), 1.5);
            }
            Kind::Teachers => {
                shape(cv, &[(-9.0, -6.4), (-1.0, -4.1), (-1.0, 7.7), (-9.0, 5.2)], first);
                shape(cv, &[(1.0, -4.1), (9.0, -6.4), (9.0, 5.2), (1.0, 7.7)], second);
                cv.line(cx, cy - 3.9, cx, cy + 8.1, solid(third), 1.35);
            }
        }
    }

    fn snowcap(&self, cv: &mut Canvas, x: f32, y: f32, w: f32) {
        let snow = c(0xf6faff);
        cv.round_rect(x + 14.0, y - 5.0, w - 28.0, 11.0, 6.0, solid(ca(0, 0, 0, 0.18)));
        cv.round_rect(x + 14.0, y - 7.0, w - 28.0, 11.0, 6.0, solid(snow));
        cv.round_rect(x + 14.0 + (w - 28.0) * 0.22, y - 1.0, 7.0, 9.0, 3.5, solid(snow));
        cv.round_rect(x + 14.0 + (w - 28.0) * 0.68, y - 1.0, 7.0, 6.0, 3.5, solid(snow));
    }

    fn wire(&self, cv: &mut Canvas, x: f32, y: f32, w: f32) {
        let n = 9;
        let sw = w / n as f32;
        let mut pb = PathBuilder::new();
        pb.move_to(x, y - 9.0);
        for i in 0..n {
            let i = i as f32;
            pb.quad_to(x + (i + 0.5) * sw, y + 1.0, x + (i + 1.0) * sw, y - 9.0);
        }
        if let Some(p) = pb.finish() {
            cv.stroke(&p, solid(c(0x1d2a24)), 1.4);
        }
    }

    fn bulbs(&self, cv: &mut Canvas, x: f32, y: f32, w: f32, t: f32) {
        let n = 9;
        let sw = w / n as f32;
        let cols = [(255u8, 90u8, 78u8), (255, 198, 90), (127, 224, 255), (127, 224, 160)];
        for i in 0..n {
            let (r, g, b) = cols[i % 4];
            let phase = (t + i as f32 * 0.37) / 1.6 * std::f32::consts::TAU;
            let a = 0.45 + 0.55 * (0.5 + 0.5 * phase.cos());
            let bx = x + (i as f32 + 0.5) * sw;
            let by = y - 1.0;
            cv.circle(bx, by, 11.0, rad(bx, by, bx, by, 11.0, &[(0.0, ca(r, g, b, 0.55 * a)), (1.0, ca(r, g, b, 0.0))]));
            cv.ellipse(bx, by, 3.5, 5.0, 0.0, solid(ca(r, g, b, a)));
        }
    }

    /// Trang trí riêng của từng chủ đề đặt trên đảo giữa (vẽ tĩnh, nằm trong khoảng trống phía trên thanh).
    fn ornament(&self, cv: &mut Canvas, x: f32, y: f32, w: f32) {
        use std::f32::consts::{PI, TAU};
        let heart = |cv: &mut Canvas, hx: f32, hy: f32, r: f32, col: Color| {
            let mut p = PathBuilder::new();
            p.move_to(hx, hy + r * 0.9);
            p.cubic_to(hx - r * 1.7, hy - r * 0.1, hx - r * 0.75, hy - r * 1.45, hx, hy - r * 0.45);
            p.cubic_to(hx + r * 0.75, hy - r * 1.45, hx + r * 1.7, hy - r * 0.1, hx, hy + r * 0.9);
            p.close();
            if let Some(path) = p.finish() {
                cv.fill(&path, solid(col));
            }
        };
        let flower = |cv: &mut Canvas, fx: f32, fy: f32, r: f32, petal: Color, mid: Color| {
            for i in 0..5 {
                let a = i as f32 * TAU / 5.0 - PI / 2.0;
                cv.circle(fx + a.cos() * r, fy + a.sin() * r, r * 0.92, solid(petal));
            }
            cv.circle(fx, fy, r * 0.5, solid(mid));
        };
        let star4 = |cv: &mut Canvas, sx: f32, sy: f32, r: f32, col: Color| {
            let mut p = PathBuilder::new();
            p.move_to(sx, sy - r);
            p.quad_to(sx, sy, sx + r, sy);
            p.quad_to(sx, sy, sx, sy + r);
            p.quad_to(sx, sy, sx - r, sy);
            p.quad_to(sx, sy, sx, sy - r);
            p.close();
            if let Some(path) = p.finish() {
                cv.fill(&path, solid(col));
            }
        };
        let right = x + w;
        match self.kind {
            Kind::Tet => {
                self.sprig(cv, x - 30.0, y - 30.0, c(0xffd23f), c(0xd9661c));
                // đèn lồng đỏ treo ở góc phải
                let lx = right - 26.0;
                cv.line(lx, y - 46.0, lx, y - 30.0, solid(c(0xe0b04f)), 1.2);
                cv.circle(lx, y - 18.0, 16.0, rad(lx, y - 18.0, lx, y - 18.0, 16.0, &[(0.0, ca(255, 90, 50, 0.35)), (1.0, ca(255, 90, 50, 0.0))]));
                cv.ellipse(lx, y - 19.0, 9.5, 11.0, 0.0, rad(lx - 3.0, y - 23.0, lx, y - 19.0, 11.0, &[(0.0, c(0xff6b4a)), (1.0, c(0xc0161c))]));
                cv.round_rect(lx - 5.0, y - 31.5, 10.0, 3.0, 1.0, solid(c(0xf0c050)));
                cv.round_rect(lx - 5.0, y - 9.5, 10.0, 3.0, 1.0, solid(c(0xf0c050)));
                cv.line(lx, y - 6.5, lx, y - 1.0, solid(c(0xf0c050)), 1.4);
            }
            Kind::Spring => self.sprig(cv, x - 30.0, y - 30.0, c(0xffb7cf), c(0xd94f7a)),
            Kind::Noel => {
                self.wire(cv, x, y, w);
                self.bulbs(cv, x, y, w, 0.0);
            }
            Kind::Winter => self.snowcap(cv, x, y, w),
            Kind::Moon => self.star_lantern(cv, right - 30.0, y - 52.0, 0.0),
            Kind::Summer => {
                // mặt trời nhỏ và con sóng
                let (sx, sy) = (right - 22.0, y - 18.0);
                for i in 0..10 {
                    let a = i as f32 * TAU / 10.0;
                    cv.line(sx + a.cos() * 12.0, sy + a.sin() * 12.0, sx + a.cos() * 16.5, sy + a.sin() * 16.5, solid(c(0xffc53d)), 2.0);
                }
                cv.circle(sx, sy, 9.5, rad(sx - 3.0, sy - 3.0, sx, sy, 9.5, &[(0.0, c(0xfff3b0)), (1.0, c(0xffb020))]));
                let mut p = PathBuilder::new();
                p.move_to(x + 8.0, y - 4.0);
                p.cubic_to(x + 18.0, y - 16.0, x + 30.0, y - 16.0, x + 34.0, y - 8.0);
                p.cubic_to(x + 30.0, y - 12.0, x + 24.0, y - 10.0, x + 26.0, y - 5.0);
                if let Some(path) = p.finish() {
                    cv.stroke(&path, solid(c(0x4dabf7)), 3.0);
                }
            }
            Kind::Autumn => {
                for (dx, dy, r, a, col) in [(6.0f32, -14.0f32, 12.0f32, -0.3f32, 0xe8590c), (26.0, -22.0, 9.0, 0.5, 0xf59f00), (44.0, -9.0, 7.0, -0.8, 0xc92a2a)] {
                    let (lx, ly) = (x + dx, y + dy);
                    if let Some(p) = star_path(lx, ly, r, r * 0.48, 5) {
                        let t = Transform::from_rotate_at(a.to_degrees(), lx, ly);
                        if let Some(p) = p.transform(t) {
                            cv.fill(&p, solid(c(col)));
                        }
                    }
                    cv.line(lx, ly + r * 0.3, lx + a.sin() * 3.0, ly + r * 1.1, solid(c(0x6b3a1a)), 1.3);
                }
            }
            Kind::NewYear => {
                for (dx, dy, r, col) in [(10.0f32, -16.0f32, 9.0f32, 0xffd66b), (28.0, -30.0, 6.0, 0xfff1c2), (40.0, -12.0, 4.5, 0xffb86b)] {
                    star4(cv, x + dx, y + dy, r, c(col));
                }
                for (dx, dy, r, col) in [(-30.0f32, -16.0f32, 8.0f32, 0xffd66b), (-12.0, -28.0, 5.0, 0xfff1c2)] {
                    star4(cv, right + dx, y + dy, r, c(col));
                }
            }
            Kind::Valentine => {
                heart(cv, x + 14.0, y - 14.0, 9.0, c(0xff4d7a));
                heart(cv, x + 32.0, y - 26.0, 6.0, c(0xff8fab));
                heart(cv, right - 20.0, y - 14.0, 7.5, c(0xff6b9a));
            }
            Kind::Women | Kind::VnWomen => {
                cv.line(x + 14.0, y - 2.0, x + 22.0, y - 20.0, solid(c(0x2f9e44)), 2.0);
                cv.ellipse(x + 14.0, y - 10.0, 5.0, 2.6, -0.6, solid(c(0x40c057)));
                flower(cv, x + 22.0, y - 22.0, 6.0, c(if self.kind == Kind::Women { 0xf06595 } else { 0xcc5de8 }), c(0xffd43b));
                flower(cv, x + 40.0, y - 12.0, 4.5, c(0xffa8c5), c(0xffe066));
            }
            Kind::Reunify | Kind::National => {
                // lá cờ đỏ sao vàng nhỏ
                let (px, py) = (x + 14.0, y - 2.0);
                cv.line(px, py, px, py - 44.0, solid(c(0xd0d4da)), 2.0);
                cv.rect(px + 1.0, py - 44.0, 30.0, 20.0, solid(c(0xda251d)));
                if let Some(p) = star_path(px + 16.0, py - 34.0, 6.5, 2.6, 5) {
                    cv.fill(&p, solid(c(0xffde00)));
                }
            }
            Kind::Children => {
                for (dx, top, col) in [(10.0f32, 46.0f32, 0xff6b6b), (24.0, 52.0, 0x4dabf7), (38.0, 42.0, 0xffd43b)] {
                    let bx = x + dx;
                    cv.line(x + 22.0, y - 1.0, bx, y - top + 12.0, solid(ca(90, 90, 90, 0.8)), 1.0);
                    cv.ellipse(bx, y - top + 2.0, 7.0, 9.0, 0.0, rad(bx - 2.5, y - top - 2.0, bx, y - top + 2.0, 9.0, &[(0.0, c(0xffffff)), (0.35, c(col)), (1.0, c(col))]));
                }
            }
            Kind::Halloween => {
                // bí ngô ở góc trái, mạng nhện ở góc phải
                let (px, py) = (x + 20.0, y - 11.0);
                for (dx, k) in [(-5.0f32, 0.62f32), (5.0, 0.62), (0.0, 0.72)] {
                    cv.ellipse(px + dx, py, 13.0 * k, 10.0, 0.0, solid(c(0xf07a16)));
                }
                cv.rect(px - 1.5, py - 13.0, 3.0, 4.0, solid(c(0x3f6b25)));
                for dx in [-4.0f32, 4.0] {
                    cv.circle(px + dx, py - 2.0, 1.8, solid(c(0xffd65a)));
                }
                cv.ellipse(px, py + 4.0, 5.0, 1.6, 0.0, solid(c(0xffd65a)));
                let (wx, wy) = (right - 16.0, y + 2.0);
                let web = ca(230, 230, 240, 0.65);
                for i in 0..5 {
                    let a = PI + i as f32 * (PI / 2.0) / 4.0;
                    cv.line(wx, wy, wx + a.cos() * 30.0, wy + a.sin() * 30.0, solid(web), 0.8);
                }
                for rr in [10.0f32, 18.0, 26.0] {
                    let mut p = PathBuilder::new();
                    for i in 0..5 {
                        let a = PI + i as f32 * (PI / 2.0) / 4.0;
                        let (px2, py2) = (wx + a.cos() * rr, wy + a.sin() * rr);
                        if i == 0 { p.move_to(px2, py2) } else { p.line_to(px2, py2) }
                    }
                    if let Some(path) = p.finish() {
                        cv.stroke(&path, solid(web), 0.8);
                    }
                }
            }
            Kind::Teachers => {
                // cuốn sách mở và bông hoa
                let (bx, by) = (x + 26.0, y - 6.0);
                for (sgn, col) in [(-1.0f32, 0xf8f9fa), (1.0, 0xe9ecef)] {
                    let mut p = PathBuilder::new();
                    p.move_to(bx, by);
                    p.quad_to(bx + sgn * 9.0, by - 5.0, bx + sgn * 18.0, by - 2.0);
                    p.line_to(bx + sgn * 18.0, by - 16.0);
                    p.quad_to(bx + sgn * 9.0, by - 19.0, bx, by - 14.0);
                    p.close();
                    if let Some(path) = p.finish() {
                        cv.fill(&path, solid(c(col)));
                        cv.stroke(&path, solid(c(0x1c7ed6)), 1.0);
                    }
                }
                cv.line(bx, by, bx, by - 14.0, solid(c(0x1c7ed6)), 1.0);
                flower(cv, right - 22.0, y - 16.0, 5.0, c(0xff8787), c(0xffd43b));
                cv.line(right - 22.0, y - 10.0, right - 22.0, y - 1.0, solid(c(0x2f9e44)), 1.6);
            }
        }
    }

    fn sprig(&self, cv: &mut Canvas, x: f32, y: f32, petal: Color, mid: Color) {
        let t = Transform::from_translate(x, y);
        let mut pb = PathBuilder::new();
        pb.move_to(4.0, 40.0);
        pb.cubic_to(24.0, 32.0, 40.0, 22.0, 66.0, 8.0);
        if let Some(p) = pb.finish().and_then(|p| p.transform(t)) {
            cv.stroke(&p, solid(c(0x4a2414)), 3.2);
        }
        let mut pb = PathBuilder::new();
        pb.move_to(34.0, 25.0);
        pb.cubic_to(42.0, 33.0, 52.0, 36.0, 62.0, 33.0);
        if let Some(p) = pb.finish().and_then(|p| p.transform(t)) {
            cv.stroke(&p, solid(c(0x4a2414)), 2.0);
        }
        for (bx, by, s) in [(66.0f32, 8.0f32, 1.0f32), (50.0, 17.0, 0.9), (62.0, 33.0, 0.85), (26.0, 31.0, 0.8), (80.0, 4.0, 0.55)] {
            for i in 0..5 {
                let a = i as f32 * std::f32::consts::TAU / 5.0;
                cv.circle(x + bx + a.cos() * 3.6 * s, y + by + a.sin() * 3.6 * s, 3.3 * s, solid(petal));
            }
            cv.circle(x + bx, y + by, 1.7 * s, solid(mid));
        }
    }

    fn star_lantern(&self, cv: &mut Canvas, x: f32, y: f32, t: f32) {
        let deg = 5.0 * (t / 3.2 * std::f32::consts::TAU).sin();
        let tr = Transform::from_translate(x, y).pre_translate(8.0, 64.0).pre_rotate(deg).pre_translate(-8.0, -64.0);
        let mut pb = PathBuilder::new();
        pb.move_to(8.0, 64.0);
        pb.line_to(22.0, 26.0);
        if let Some(p) = pb.finish().and_then(|p| p.transform(tr)) {
            cv.stroke(&p, solid(c(0xc99a5a)), 2.2);
        }
        let center = tr.map_point_helper(22.0, 18.5);
        cv.circle(center.0, center.1, 24.0, rad(center.0, center.1, center.0, center.1, 24.0, &[(0.0, ca(255, 170, 80, 0.4)), (1.0, ca(255, 170, 80, 0.0))]));
        if let Some(p) = star_path(22.0, 18.8, 11.5, 5.2, 5).and_then(|p| p.transform(tr)) {
            cv.fill(&p, solid(c(0xff9e3d)));
            cv.stroke(&p, solid(c(0xffe0a3)), 1.2);
        }
        cv.circle(center.0, center.1, 3.4, solid(c(0xfff1c2)));
    }

    /// Tắt taskbar riêng và trả taskbar gốc của Windows về như cũ.
    pub fn shutdown(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        shell::destroy_popup(self.flyout.hwnd);
        shell::destroy_popup(self.cal.hwnd);
        self.menu.close();
        shell::destroy_popup(self.menu.hwnd);
        self.preview.close();
        shell::destroy_popup(self.preview.hwnd);
        shell::appbar_remove(self.hwnd);
        shell::release_work_area(self.mon);
        shell::show(self.hwnd, false);
        crate::log("đã tắt taskbar riêng trên một màn hình");
    }
}

trait MapPoint {
    fn map_point_helper(&self, x: f32, y: f32) -> (f32, f32);
}

impl MapPoint for Transform {
    fn map_point_helper(&self, x: f32, y: f32) -> (f32, f32) {
        (self.sx * x + self.kx * y + self.tx, self.ky * x + self.sy * y + self.ty)
    }
}

impl Drop for Bar {
    fn drop(&mut self) {
        self.shutdown();
    }
}
