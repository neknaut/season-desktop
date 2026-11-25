//! Icon khay hệ thống: bấm chuột trái mở cài đặt, chuột phải hiện menu.

use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct Tray {
    _icon: TrayIcon,
    pub settings: MenuItem,
    pub pause: CheckMenuItem,
    pub quit: MenuItem,
}

impl Tray {
    pub fn new() -> Result<Self, String> {
        let settings = MenuItem::new("Mở cài đặt", true, None);
        let pause = CheckMenuItem::new("Tạm dừng hình nền", true, false, None);
        let quit = MenuItem::new("Thoát Season Desktop", true, None);
        let menu = Menu::new();
        menu.append_items(&[&settings, &pause, &PredefinedMenuItem::separator(), &quit]).map_err(|e| e.to_string())?;
        let (rgba, n) = icon_rgba(32);
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Season Desktop")
            .with_icon(Icon::from_rgba(rgba, n, n).map_err(|e| e.to_string())?)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { _icon: icon, settings, pause, quit })
    }

    pub fn set_visible(&self, on: bool) {
        let _ = self._icon.set_visible(on);
    }
}

/// Render the same icon for the tray, settings, window and exported ICO.
fn icon_pixmap(n: u32) -> tiny_skia::Pixmap {
    use crate::paint::{c, lin, solid, Canvas};
    use tiny_skia::{PathBuilder, Transform};

    // Rasterize each frame directly at its target size, including small Windows tray sizes.
    let mut cv = Canvas::new(n, n, Transform::from_scale(n as f32 / 64.0, n as f32 / 64.0));
    let zoom = if n <= 16 { 1.02 } else { 1.08 };
    cv.base = cv.base.pre_translate(31.6, 32.0).pre_scale(zoom, zoom).pre_translate(-32.0, -32.0);
    let base = cv.base;
    let small = n <= 24;
    for (x, angle, color) in [(9.0, -22.0, 0x58d3ba), (24.0, 22.0, 0x9179f3)] {
        cv.base = base.pre_rotate_at(angle, x + 16.0, 32.0);
        cv.round_rect(x, 12.0, 32.0, 42.0, 7.0, solid(c(0x283253)));
        cv.round_rect(x + 2.0, 14.0, 28.0, 38.0, 5.0, solid(c(color)));
    }
    cv.base = base;

    let mut sheet = PathBuilder::new();
    sheet.move_to(23.0, 6.0);
    sheet.line_to(38.0, 6.0);
    sheet.line_to(50.0, 18.0);
    sheet.line_to(50.0, 53.0);
    sheet.quad_to(50.0, 58.0, 45.0, 58.0);
    sheet.line_to(23.0, 58.0);
    sheet.quad_to(17.0, 58.0, 17.0, 52.0);
    sheet.line_to(17.0, 12.0);
    sheet.quad_to(17.0, 6.0, 23.0, 6.0);
    sheet.close();
    if let Some(path) = sheet.finish() {
        let paint = if small { solid(c(0xffbf71)) } else {
            lin(21.0, 16.0, 51.0, 55.0, &[(0.0, c(0xffd087)), (1.0, c(0xf78b82))])
        };
        cv.stroke(&path, solid(c(0x283253)), 2.5);
        cv.fill(&path, paint);
    }

    let mut fold = PathBuilder::new();
    fold.move_to(38.0, 6.0);
    fold.line_to(50.0, 18.0);
    fold.line_to(41.0, 18.0);
    fold.quad_to(38.0, 18.0, 38.0, 15.0);
    fold.close();
    if let Some(path) = fold.finish() {
        cv.fill(&path, solid(c(0xffecc6)));
    }

    let mut moon = PathBuilder::new();
    moon.move_to(33.5, 26.0);
    moon.cubic_to(31.7, 33.4, 36.0, 37.8, 42.0, 37.2);
    moon.cubic_to(40.7, 43.5, 33.3, 45.7, 28.9, 41.5);
    moon.cubic_to(23.6, 36.5, 26.2, 28.1, 33.5, 26.0);
    moon.close();
    if let Some(path) = moon.finish() {
        cv.fill(&path, solid(c(0x583869)));
    }
    cv.pm
}

pub fn icon_rgba(n: u32) -> (Vec<u8>, u32) {
    let pixmap = icon_pixmap(n);
    let mut rgba = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.pixels() {
        let pixel = pixel.demultiply();
        rgba.extend_from_slice(&[pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]);
    }
    (rgba, n)
}

/// Icon của app dạng file .ico (nhiều cỡ, mỗi cỡ là một ảnh PNG).
pub fn icon_ico() -> Vec<u8> {
    let sizes = [16u32, 20, 24, 32, 40, 48, 64, 96, 128, 256];
    let mut images: Vec<Vec<u8>> = Vec::new();
    for &n in &sizes {
        images.push(icon_pixmap(n).encode_png().expect("PNG icon encoding"));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (i, &n) in sizes.iter().enumerate() {
        let d = if n >= 256 { 0 } else { n as u8 };
        out.extend_from_slice(&[d, d, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(images[i].len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += images[i].len() as u32;
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}
