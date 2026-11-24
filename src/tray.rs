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
    use crate::paint::{c, ca, lin, solid, Canvas};
    use tiny_skia::{FilterQuality, PathBuilder, Pixmap, PixmapPaint, Transform};

    const SS: u32 = 4;
    let size = n * SS;
    let mut cv = Canvas::new(size, size, Transform::from_scale(size as f32 / 64.0, size as f32 / 64.0));
    cv.round_rect(2.0, 2.0, 60.0, 60.0, 16.0,
        lin(6.0, 4.0, 55.0, 60.0, &[(0.0, c(0x263752)), (1.0, c(0x121b30))]));
    if let Some(edge) = crate::paint::round_rect_path(2.6, 2.6, 58.8, 58.8, 15.4) {
        cv.stroke(&edge, solid(ca(196, 218, 255, 0.26)), 1.1);
    }
    let petal_tones = [
        (0xffccb4, 0xf58b87), // spring
        (0xffeb99, 0xf6bb54), // summer
        (0xffc182, 0xe97756), // autumn
        (0xbbf7f0, 0x59b9ce), // winter
    ];
    for (i, (high, low)) in petal_tones.into_iter().enumerate() {
        let angle = i as f32 * std::f32::consts::FRAC_PI_2 + 0.12;
        let original = cv.base;
        cv.base = original.pre_translate(32.0, 32.0).pre_rotate(angle.to_degrees());
        let mut p = PathBuilder::new();
        p.move_to(2.8, -2.6);
        p.cubic_to(1.0, -10.3, 6.8, -20.7, 15.6, -20.8);
        p.cubic_to(22.6, -20.9, 23.7, -16.0, 19.1, -10.4);
        p.cubic_to(14.2, -4.7, 7.1, -3.0, 2.8, -2.6);
        p.close();
        if let Some(path) = p.finish() {
            cv.fill(&path, lin(14.0, -19.0, 5.0, -2.0, &[(0.0, c(high)), (1.0, c(low))]));
        }
        cv.base = original;
    }
    cv.circle(32.0, 32.0, 4.1, solid(c(0xfff3d5)));

    let mut small = Pixmap::new(n, n).expect("valid icon dimensions");
    small.draw_pixmap(0, 0, cv.pm.as_ref(),
        &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() },
        Transform::from_scale(1.0 / SS as f32, 1.0 / SS as f32), None);
    small
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
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
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
