//! Menu chuột phải tự vẽ theo kiểu Windows 11: nền tối/sáng theo Windows, bo góc 8 px,
//! icon Segoe Fluent bên trái, chữ Segoe UI, rê chuột có nền nhạt, bấm ra ngoài thì đóng.

use ab_glyph::FontVec;
use tiny_skia::*;
use windows::Win32::Foundation::{HWND, RECT};

use super::shell;
use crate::paint::{ca, round_rect_path, solid, Canvas};
use crate::widget;

/// Windows đang dùng tiếng Việt không (chữ trong menu theo ngôn ngữ của Windows, như taskbar gốc).
pub fn vietnamese() -> bool {
    static VI: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *VI.get_or_init(|| unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3FF == 0x2A)
}

/// Chọn chữ theo ngôn ngữ của Windows.
pub fn tr(en: &'static str, vi: &'static str) -> &'static str {
    if vietnamese() {
        vi
    } else {
        en
    }
}

/// Taskbar của Windows đang ở chế độ tối không (menu taskbar gốc theo "Chế độ Windows").
fn dark() -> bool {
    shell::reg_dword(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "SystemUsesLightTheme").map(|v| v == 0).unwrap_or(true)
}

/// Màu theo chủ đề của taskbar (giống lịch và bảng ^).
#[derive(Clone, Copy)]
pub struct Theme {
    pub tint: (u8, u8, u8, f32),
    pub border: (u8, u8, u8, f32),
    pub fg: [u8; 3],
    pub acc: [u8; 3],
    /// Viền tuyết trên đỉnh (chủ đề Giáng sinh)
    pub snow: bool,
}

pub enum Ico {
    None,
    /// Ký tự trong font Segoe Fluent Icons
    Glyph(char),
    Image(Pixmap),
}

#[derive(PartialEq, Clone, Copy)]
pub enum Kind {
    Normal,
    Header,
    Separator,
}

pub struct Item {
    pub id: u32,
    pub text: String,
    pub icon: Ico,
    pub kind: Kind,
}

impl Item {
    pub fn new(id: u32, text: impl Into<String>, icon: Ico) -> Self {
        Item { id, text: text.into(), icon, kind: Kind::Normal }
    }
    pub fn header(text: impl Into<String>) -> Self {
        Item { id: 0, text: text.into(), icon: Ico::None, kind: Kind::Header }
    }
    pub fn sep() -> Self {
        Item { id: 0, text: String::new(), icon: Ico::None, kind: Kind::Separator }
    }
}

pub struct Fonts {
    pub text: FontVec,
    pub semi: FontVec,
    pub icons: Option<FontVec>,
}

/// Font giao diện của Windows (Segoe UI) và font icon Segoe Fluent.
pub fn ui_fonts() -> Option<&'static Fonts> {
    static F: std::sync::OnceLock<Option<Fonts>> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let dir = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("Fonts");
        let load = |names: &[&str]| names.iter().find_map(|n| std::fs::read(dir.join(n)).ok().and_then(|b| FontVec::try_from_vec(b).ok()));
        let text = load(&["segoeui.ttf"]).or_else(|| FontVec::try_from_vec(widget::BE_VIETNAM_REGULAR.to_vec()).ok())?;
        let semi = load(&["seguisb.ttf"]).or_else(|| FontVec::try_from_vec(widget::BE_VIETNAM_SEMIBOLD.to_vec()).ok())?;
        let icons = load(&["SegoeIcons.ttf", "segmdl2.ttf"]);
        Some(Fonts { text, semi, icons })
    })
    .as_ref()
}

/// Kích thước theo Windows 11 (đơn vị điểm ảnh ở tỉ lệ 100%).
const SHADOW: f32 = 14.0;
const PAD: f32 = 4.0;
const ROW: f32 = 32.0;
const HEAD: f32 = 30.0;
const SEP: f32 = 9.0;
const TEXT_X: f32 = 44.0;
const MIN_W: f32 = 220.0;
const MAX_TEXT: f32 = 300.0;

pub struct Menu {
    pub hwnd: HWND,
    layered: Option<shell::Layered>,
    pub open: bool,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    k: f32,
    bw: f32,
    items: Vec<Item>,
    rows: Vec<(f32, f32)>,
    hover: Option<usize>,
    dark: bool,
    theme: Option<Theme>,
    fade: Option<shell::Fade>,
}

impl Menu {
    pub fn new(hwnd: HWND) -> Self {
        Menu { hwnd, layered: None, open: false, x: 0, y: 0, w: 0, h: 0, k: 1.0, bw: 0.0, items: Vec::new(), rows: Vec::new(), hover: None, dark: true, theme: None, fade: None }
    }

    /// Mở menu phía trên điểm (ax, top) – toạ độ màn hình – trong màn hình `mon`. `k` = tỉ lệ DPI.
    pub fn show(&mut self, items: Vec<Item>, ax: i32, top: i32, mon: RECT, k: f32, theme: Option<Theme>) {
        let Some(f) = ui_fonts() else { return };
        self.close();
        self.k = k;
        self.dark = dark();
        self.theme = theme;
        let mut y = PAD;
        let mut rows = Vec::new();
        let mut tw: f32 = 0.0;
        for it in &items {
            let h = match it.kind {
                Kind::Normal => ROW,
                Kind::Header => HEAD,
                Kind::Separator => SEP,
            };
            rows.push((y, h));
            y += h;
            if it.kind != Kind::Separator {
                let font = if it.kind == Kind::Header { &f.semi } else { &f.text };
                tw = tw.max(widget::measure(font, 14.0 * k, &it.text) / k);
            }
        }
        let bh = y + PAD;
        let bw = (TEXT_X + tw.min(MAX_TEXT) + 40.0).max(MIN_W).ceil();
        self.bw = bw;
        self.items = items;
        self.rows = rows;
        self.hover = None;
        let (w, h) = (((bw + SHADOW * 2.0) * k).ceil() as i32, ((bh + SHADOW * 2.0) * k).ceil() as i32);
        let sh = (SHADOW * k).round() as i32;
        let x = (ax - w / 2).clamp(mon.left + 4 - sh, mon.right - w - 4 + sh);
        let y = (top - (8.0 * k) as i32 - h + sh).max(mon.top - sh);
        self.x = x;
        self.y = y;
        self.w = w;
        self.h = h;
        self.layered = shell::Layered::new(w, h);
        if let Some(l) = self.layered.as_mut() {
            l.alpha = 0;
            l.dy = (shell::FADE_SLIDE * 0.6 * k) as i32;
        }
        self.fade = shell::Fade::open();
        shell::move_popup(self.hwnd, x, y, w, h);
        self.open = true;
        self.draw();
        shell::show(self.hwnd, true);
        shell::watch_outside(self.hwnd, true);
    }

    pub fn close(&mut self) {
        if self.open {
            self.open = false;
            self.fade = shell::Fade::close();
            shell::watch_outside(self.hwnd, false);
        }
    }

    pub fn animating(&self) -> bool {
        self.fade.is_some()
    }

    /// Một bước hiệu ứng mở/đóng. Trả về true nếu còn chạy.
    pub fn animate(&mut self) -> bool {
        shell::fade_step(&mut self.fade, &mut self.layered, self.hwnd, self.k * 0.6)
    }

    fn row_at(&self, x: i32, y: i32) -> Option<usize> {
        let (lx, ly) = (x as f32 / self.k - SHADOW, y as f32 / self.k - SHADOW);
        if lx < 0.0 || lx > self.bw {
            return None;
        }
        self.rows.iter().position(|(t, h)| ly >= *t && ly < t + h).filter(|i| self.items[*i].kind == Kind::Normal)
    }

    /// Xử lý chuột. Trả về mã của mục được chọn (menu tự đóng).
    pub fn mouse(&mut self, e: shell::PopupEvent) -> Option<u32> {
        if !self.open {
            return None;
        }
        match e {
            shell::PopupEvent::Move(x, y) => {
                let h = self.row_at(x, y);
                if h != self.hover {
                    self.hover = h;
                    self.draw();
                }
            }
            shell::PopupEvent::Leave => {
                if self.hover.take().is_some() {
                    self.draw();
                }
            }
            shell::PopupEvent::Up { .. } => {
                if let Some(i) = self.hover {
                    let id = self.items[i].id;
                    self.close();
                    return Some(id);
                }
            }
            shell::PopupEvent::Outside => self.close(),
        }
        None
    }

    fn draw(&mut self) {
        let Some(f) = ui_fonts() else { return };
        if self.w <= 0 || self.h <= 0 {
            return;
        }
        let k = self.k;
        let (bw, bh) = (self.bw, self.h as f32 / k - SHADOW * 2.0);
        let mut cv = Canvas::new(self.w as u32, self.h as u32, Transform::from_scale(k, k));
        // Bóng mờ quanh menu
        for i in 0..10 {
            let s = i as f32 * 1.3;
            if let Some(p) = round_rect_path(SHADOW - s, SHADOW - s + 3.0, bw + s * 2.0, bh + s * 2.0, 8.0 + s) {
                cv.fill(&p, solid(ca(0, 0, 0, if self.dark { 0.05 } else { 0.022 })));
            }
        }
        let (bg, border, fg, hover, line, sub) = if self.dark {
            (ca(44, 44, 44, 1.0), ca(255, 255, 255, 0.09), [255, 255, 255], ca(255, 255, 255, 0.07), ca(255, 255, 255, 0.09), 0.72)
        } else {
            (ca(249, 249, 249, 1.0), ca(0, 0, 0, 0.1), [26, 26, 26], ca(0, 0, 0, 0.055), ca(0, 0, 0, 0.08), 0.62)
        };
        let th = self.theme;
        let (border, fg, hover, line, icon_rgb) = match th {
            Some(t) => (ca(t.border.0, t.border.1, t.border.2, t.border.3), t.fg, ca(255, 255, 255, 0.12), ca(t.border.0, t.border.1, t.border.2, t.border.3 * 0.6), t.acc),
            None => (border, fg, hover, line, fg),
        };
        if let Some(p) = round_rect_path(SHADOW + 0.5, SHADOW + 0.5, bw - 1.0, bh - 1.0, 8.0) {
            match th {
                // Nền đặc tối rồi phủ màu chủ đề (như lịch của taskbar)
                Some(t) => {
                    cv.fill(&p, solid(ca(16, 18, 26, 1.0)));
                    cv.fill(&p, solid(ca(t.tint.0, t.tint.1, t.tint.2, t.tint.3)));
                }
                None => cv.fill(&p, solid(bg)),
            }
            cv.stroke(&p, solid(border), 1.0);
        }
        if th.is_some_and(|t| t.snow) {
            let snow = ca(0xf6, 0xfa, 0xff, 1.0);
            cv.round_rect(SHADOW + 10.0, SHADOW - 4.0, bw - 20.0, 7.0, 3.5, solid(snow));
            cv.round_rect(SHADOW + (bw - 20.0) * 0.72, SHADOW, 5.0, 6.0, 2.5, solid(snow));
        }
        let mut texts: Vec<(u8, f32, f32, f32, f32, String)> = Vec::new(); // (font: 0 chữ, 1 đậm, 2 icon), px, x, baseline, alpha, chữ
        let mut images: Vec<(f32, f32, usize)> = Vec::new();
        for (i, it) in self.items.iter().enumerate() {
            let (top, h) = self.rows[i];
            let (ox, oy) = (SHADOW, SHADOW + top);
            match it.kind {
                Kind::Separator => cv.rect(ox + 1.0, oy + (h / 2.0).floor(), bw - 2.0, 1.0, solid(line)),
                Kind::Header => {
                    let t = clip(&f.semi, 14.0 * k, &it.text, MAX_TEXT * k);
                    texts.push((1, 14.0, ox + 12.0, oy + h / 2.0 + 6.0, sub, t));
                }
                Kind::Normal => {
                    if self.hover == Some(i) {
                        cv.round_rect(ox + 4.0, oy + 2.0, bw - 8.0, h - 4.0, 4.0, solid(hover));
                    }
                    match &it.icon {
                        Ico::Glyph(g) => texts.push((2, 16.0, ox + 12.0, oy + h / 2.0 + 8.0, 1.0, g.to_string())),
                        Ico::Image(_) => images.push((ox + 12.0, oy + h / 2.0 - 8.0, i)),
                        Ico::None => {}
                    }
                    let t = clip(&f.text, 14.0 * k, &it.text, MAX_TEXT * k);
                    texts.push((0, 14.0, ox + TEXT_X, oy + h / 2.0 + 5.0, 1.0, t));
                }
            }
        }
        let mut pm = cv.pm;
        for (x, y, i) in images {
            if let Ico::Image(img) = &self.items[i].icon {
                let s = (16.0 * k) / img.width().max(1) as f32;
                pm.draw_pixmap(0, 0, img.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, Transform::from_translate((x * k).round(), (y * k).round()).pre_scale(s, s), None);
            }
        }
        for (font, px, x, bl, a, s) in &texts {
            let fo = match font {
                0 => &f.text,
                1 => &f.semi,
                _ => match &f.icons {
                    Some(ic) => ic,
                    None => continue,
                },
            };
            let rgb = match font {
                2 => icon_rgb,
                1 if th.is_some() => icon_rgb,
                _ => fg,
            };
            widget::text(&mut pm, fo, px * k, x * k, bl * k, rgb, *a, s);
        }
        let (x, y, hwnd) = (self.x, self.y, self.hwnd);
        if let Some(l) = &mut self.layered {
            l.present(hwnd, x, y, pm.data());
        }
    }
}

/// Cắt chữ quá dài, thêm "…".
fn clip(font: &FontVec, px: f32, s: &str, max: f32) -> String {
    if widget::measure(font, px, s) <= max {
        return s.to_string();
    }
    let mut out: String = s.to_string();
    while !out.is_empty() && widget::measure(font, px, &(out.clone() + "…")) > max {
        out.pop();
    }
    out + "…"
}
