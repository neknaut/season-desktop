//! Đồng hồ, ngày, lời chúc và đếm ngược vẽ trên hình nền (như bản xem thử).
//! Chỉ vẽ lại khi đổi phút hoặc đổi theme, nên gần như không tốn gì.

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use tiny_skia::{Pixmap, PremultipliedColorU8, Transform};

use crate::desktop::LocalTime;
use crate::paint::{ca, round_rect_path, solid, Canvas};
use crate::scenes::Kind;

const MAX_WIDGET_W: f32 = 560.0;

// Font giống bản xem thử, nhúng sẵn trong file exe (giấy phép OFL, xem assets/fonts).
pub static BE_VIETNAM_REGULAR: &[u8] = include_bytes!("../assets/fonts/BeVietnamPro-Regular.ttf");
pub static BE_VIETNAM_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/BeVietnamPro-SemiBold.ttf");
pub static DANCING_SCRIPT: &[u8] = include_bytes!("../assets/fonts/DancingScript.ttf");

pub struct Fonts {
    /// Be Vietnam Pro 400 cho ô đếm ngược
    regular: FontVec,
    /// Dancing Script 600 cho lời chúc
    script: FontVec,
}

impl Fonts {
    pub fn load() -> Option<Self> {
        use ab_glyph::VariableFont;
        let f = |b: &[u8]| FontVec::try_from_vec(b.to_vec()).ok();
        let mut script = f(DANCING_SCRIPT)?;
        script.set_variation(b"wght", 600.0);
        Some(Self { regular: f(BE_VIETNAM_REGULAR)?, script })
    }
}

/// Vị trí góc trên trái của widget theo toạ độ thiết kế.
pub fn position(kind: Kind, l: crate::scenes::Layout, painted: bool) -> (f32, f32) {
    match (kind, l.portrait()) {
        (Kind::Tet, false) if !painted => (64.0, 300.0),
        (Kind::Tet, true) if !painted => (48.0, l.h * 0.575),
        (_, false) => (64.0, 52.0),
        (_, true) => (48.0, 72.0),
    }
}

struct Palette {
    fg: [u8; 3],
    accent: [u8; 3],
    chip: (u8, u8, u8, f32),
    border: (u8, u8, u8, f32),
}

fn palette(kind: Kind) -> Palette {
    match kind {
        Kind::Noel => Palette { fg: [241, 246, 255], accent: [255, 210, 122], chip: (8, 38, 44, 0.45), border: (170, 235, 215, 0.30) },
        Kind::Moon => Palette { fg: [247, 243, 255], accent: [255, 213, 138], chip: (18, 20, 60, 0.45), border: (255, 214, 150, 0.32) },
        k => match k.clock_on_light() {
            // Nền trời sáng: chữ tối trên nền chip sáng
            Some((fg, acc)) => Palette { fg, accent: acc, chip: (255, 255, 255, 0.55), border: (fg[0], fg[1], fg[2], 0.18) },
            None => {
                let t = k.theme();
                Palette { fg: t.fg, accent: t.acc, chip: (t.bar_bg.0, t.bar_bg.1, t.bar_bg.2, 0.45), border: t.bar_border }
            }
        },
    }
}

const WEEKDAYS: [&str; 7] = ["Chủ Nhật", "Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"];
const TETS: [(i32, u32, u32, &str); 3] = [(2027, 2, 6, "Đinh Mùi"), (2028, 1, 26, "Mậu Thân"), (2029, 2, 13, "Kỷ Dậu")];

/// Số ngày kể từ 1970-01-01 (thuật toán của Howard Hinnant).
pub fn days(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn tet_dates() -> &'static [(i32, u32, u32, &'static str)] {
    &TETS
}

pub fn greeting(kind: Kind) -> &'static str {
    kind.greeting()
}

pub fn chip_text(kind: Kind, now: &LocalTime) -> String {
    let today = days(now.year, now.month, now.day);
    match kind {
        Kind::Noel => {
            let mut d = days(now.year, 12, 25) - today;
            if d < 0 {
                d = days(now.year + 1, 12, 25) - today;
            }
            if d == 0 { "Hôm nay là Giáng sinh".into() } else { format!("Còn {d} ngày tới Giáng sinh") }
        }
        Kind::Tet => {
            for (y, m, dd, name) in TETS {
                let d = days(y, m, dd) - today;
                if d >= 0 {
                    return if d == 0 { format!("Chúc mừng năm mới {name}") } else { format!("Còn {d} ngày tới Tết {name}") };
                }
            }
            "Chúc mừng năm mới".into()
        }
        Kind::Moon => "Rằm tháng Tám".into(),
        // Mùa: tên mùa + tháng; ngày lễ: đếm ngược tới ngày chính
        k if !k.is_holiday() => format!("{} · tháng {}", k.name(), now.month),
        k => match crate::config::events(now.year).into_iter().filter(|e| e.2 == k && e.3 >= today).min_by_key(|e| e.3) {
            Some(e) if e.3 == today => format!("Hôm nay: {}", k.name()),
            Some(e) => format!("Còn {} ngày tới {}", e.3 - today, k.name()),
            None => k.name().to_string(),
        },
    }
}

/// Khoá để biết khi nào cần vẽ lại.
pub fn key(kind: Kind, now: &LocalTime) -> String {
    format!("{kind:?}{}:{}-{}", now.hour, now.minute, now.day)
}

fn blend(pm: &mut Pixmap, x: i32, y: i32, rgb: [u8; 3], a: f32) {
    if x < 0 || y < 0 || x >= pm.width() as i32 || y >= pm.height() as i32 || a <= 0.0 {
        return;
    }
    let i = (y as u32 * pm.width() + x as u32) as usize;
    let px = &mut pm.pixels_mut()[i];
    let inv = 1.0 - a;
    let mix = |s: u8, d: u8| ((s as f32 * a) + d as f32 * inv).round().min(255.0) as u8;
    let out_a = ((a * 255.0) + px.alpha() as f32 * inv).round().min(255.0) as u8;
    let (r, g, b) = (mix(rgb[0], px.red()), mix(rgb[1], px.green()), mix(rgb[2], px.blue()));
    if let Some(p) = PremultipliedColorU8::from_rgba(r.min(out_a), g.min(out_a), b.min(out_a), out_a) {
        *px = p;
    }
}

pub fn measure(font: &FontVec, px: f32, text: &str) -> f32 {
    let sf = font.as_scaled(PxScale::from(px));
    let mut w = 0.0;
    let mut last = None;
    for ch in text.chars() {
        let id = sf.glyph_id(ch);
        if let Some(l) = last {
            w += sf.kern(l, id);
        }
        w += sf.h_advance(id);
        last = Some(id);
    }
    w
}

pub fn text(pm: &mut Pixmap, font: &FontVec, px: f32, x: f32, baseline: f32, rgb: [u8; 3], alpha: f32, s: &str) {
    let sf = font.as_scaled(PxScale::from(px));
    let mut caret = x;
    let mut last = None;
    for ch in s.chars() {
        let id = sf.glyph_id(ch);
        if id.0 == 0 && !ch.is_whitespace() {
            // Font thiếu chữ này: bỏ qua thay vì vẽ ô vuông.
            continue;
        }
        if let Some(l) = last {
            caret += sf.kern(l, id);
        }
        let glyph = id.with_scale_and_position(PxScale::from(px), ab_glyph::point(caret, baseline));
        caret += sf.h_advance(id);
        last = Some(id);
        if let Some(o) = font.outline_glyph(glyph) {
            let b = o.px_bounds();
            o.draw(|gx, gy, cov| blend(pm, b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, rgb, cov * alpha));
        }
    }
}

pub fn render(fonts: &Fonts, kind: Kind, now: &LocalTime, k: f32) -> Pixmap {
    let pal = palette(kind);
    let time = format!("{:02}:{:02}", now.hour, now.minute);
    let date = format!("{}, {} tháng {}", WEEKDAYS[(now.weekday % 7) as usize], now.day, now.month);
    let message = greeting(kind);
    let chip = chip_text(kind, now);

    let font_width = |font: &FontVec, px: f32, text: &str| measure(font, px * k, text) / k;
    let pad = 15.0;
    let chip_px = 15.0;
    let chip_w = font_width(&fonts.regular, chip_px, &chip) + 56.0;
    let message_limit = MAX_WIDGET_W - 2.0 * pad - 4.0;
    let message_px = 39.0 * (message_limit / font_width(&fonts.script, 39.0, message).max(1.0)).min(1.0);
    let width = font_width(&fonts.regular, 94.0, &time)
        .max(font_width(&fonts.regular, 20.0, &date))
        .max(font_width(&fonts.script, message_px, message))
        .max(chip_w);
    let width = ((width + pad * 2.0 + 5.0).min(MAX_WIDGET_W) * k).ceil() as u32;
    let mut cv = Canvas::new(width, (232.0 * k).ceil() as u32, Transform::from_scale(k, k));
    let halo = if kind.clock_on_light().is_some() { [255, 248, 231] } else { [13, 18, 33] };

    let (cx, cy, ch) = (pad, 190.0, 36.0);
    let (br, bg, bb, ba) = pal.border;
    let (fr, fg, fb, fa) = pal.chip;
    cv.round_rect(cx, cy, chip_w, ch, ch / 2.0, solid(ca(br, bg, bb, ba)));
    cv.round_rect(cx + 1.0, cy + 1.0, chip_w - 2.0, ch - 2.0, ch / 2.0 - 1.0, solid(ca(fr, fg, fb, fa.max(0.45))));
    let ic = ca(pal.fg[0], pal.fg[1], pal.fg[2], 1.0);
    if let Some(p) = round_rect_path(cx + 16.0, cy + 11.0, 16.0, 15.0, 2.0) {
        cv.stroke(&p, solid(ic), 1.6);
    }
    cv.line(cx + 16.0, cy + 16.0, cx + 32.0, cy + 16.0, solid(ic), 1.6);
    cv.line(cx + 21.0, cy + 8.5, cx + 21.0, cy + 12.0, solid(ic), 1.6);
    cv.line(cx + 27.0, cy + 8.5, cx + 27.0, cy + 12.0, solid(ic), 1.6);

    let mut pm = cv.pm;
    let lines = [
        (&fonts.regular, 94.0, 99.0, pal.fg, time.as_str()),
        (&fonts.regular, 20.0, 131.0, pal.fg, date.as_str()),
        (&fonts.script, message_px, 174.0, pal.accent, message),
    ];
    for (font, px, baseline, ink, value) in lines {
        text(&mut pm, font, px * k, (pad - 1.1) * k, (baseline + 1.0) * k, halo, 0.77, value);
        text(&mut pm, font, px * k, (pad + 1.1) * k, (baseline + 1.6) * k, halo, 0.73, value);
        text(&mut pm, font, px * k, pad * k, baseline * k, ink, 1.0, value);
    }
    text(&mut pm, &fonts.regular, chip_px * k, (cx + 40.0) * k, (cy + 23.5) * k, pal.fg, 1.0, &chip);
    pm
}
