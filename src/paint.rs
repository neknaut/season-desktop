//! Vẽ 2D bằng CPU (tiny-skia) cho các lớp tĩnh. API mô phỏng canvas của bản xem thử,
//! toạ độ thiết kế 1280x720 được phóng theo độ phân giải màn hình.

use tiny_skia::*;

pub use crate::draw::{H, TAU, W};

pub struct Canvas {
    pub pm: Pixmap,
    pub base: Transform,
}

pub fn c(hex: u32) -> Color {
    Color::from_rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
}

pub fn ca(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::from_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

pub fn solid(col: Color) -> Shader<'static> {
    Shader::SolidColor(col)
}

fn stops(st: &[(f32, Color)]) -> Vec<GradientStop> {
    st.iter().map(|(p, col)| GradientStop::new(*p, *col)).collect()
}

pub fn lin(x0: f32, y0: f32, x1: f32, y1: f32, st: &[(f32, Color)]) -> Shader<'static> {
    LinearGradient::new(Point::from_xy(x0, y0), Point::from_xy(x1, y1), stops(st), SpreadMode::Pad, Transform::identity())
        .unwrap_or(Shader::SolidColor(st[0].1))
}

/// Gradient tròn: tiêu điểm (x0, y0), tâm (x1, y1), bán kính r.
pub fn rad(x0: f32, y0: f32, x1: f32, y1: f32, r: f32, st: &[(f32, Color)]) -> Shader<'static> {
    rad_t(x0, y0, x1, y1, r, st, Transform::identity())
}

pub fn rad_t(x0: f32, y0: f32, x1: f32, y1: f32, r: f32, st: &[(f32, Color)], t: Transform) -> Shader<'static> {
    RadialGradient::new(Point::from_xy(x0, y0), Point::from_xy(x1, y1), r.max(0.01), stops(st), SpreadMode::Pad, t)
        .unwrap_or(Shader::SolidColor(st[st.len() - 1].1))
}

impl Canvas {
    pub fn new(w: u32, h: u32, base: Transform) -> Self {
        Self { pm: Pixmap::new(w.max(1), h.max(1)).expect("pixmap"), base }
    }

    fn paint(sh: Shader<'_>) -> Paint<'_> {
        Paint { shader: sh, anti_alias: true, ..Default::default() }
    }

    pub fn fill(&mut self, p: &Path, sh: Shader<'_>) {
        self.pm.fill_path(p, &Self::paint(sh), FillRule::Winding, self.base, None);
    }

    /// Tô hình đã đặt trong hệ toạ độ riêng `t` (cả hình lẫn gradient).
    pub fn fill_t(&mut self, p: &Path, sh: Shader<'static>, t: Transform) {
        self.pm.fill_path(p, &Self::paint(sh), FillRule::Winding, self.base.pre_concat(t), None);
    }

    pub fn stroke(&mut self, p: &Path, sh: Shader<'static>, width: f32) {
        let st = Stroke { width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
        self.pm.stroke_path(p, &Self::paint(sh), &st, self.base, None);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, sh: Shader<'_>) {
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            self.pm.fill_rect(r, &Self::paint(sh), self.base, None);
        }
    }

    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, sh: Shader<'_>) {
        if let Some(p) = round_rect_path(x, y, w, h, r) {
            self.fill(&p, sh);
        }
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, sh: Shader<'_>) {
        if let Some(p) = PathBuilder::from_circle(cx, cy, r.max(0.01)) {
            self.fill(&p, sh);
        }
    }

    /// Ellipse xoay `rot` (radian) quanh tâm. Gradient tính theo toạ độ của canvas.
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, sh: Shader<'_>) {
        if let Some(p) = ellipse_path(cx, cy, rx, ry, rot) {
            self.fill(&p, sh);
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, sh: Shader<'static>, width: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        if let Some(p) = pb.finish() {
            self.stroke(&p, sh, width);
        }
    }

    /// Quầng sáng giống sprite glow trong bản xem thử, vẽ thẳng vào ảnh tĩnh.
    pub fn glow(&mut self, cx: f32, cy: f32, size: f32, rgb: (u8, u8, u8), a: f32) {
        let (r, g, b) = rgb;
        let sh = rad(cx, cy, cx, cy, size / 2.0, &[
            (0.0, ca(r, g, b, a)),
            (0.18, ca(r, g, b, 0.6 * a)),
            (0.5, ca(r, g, b, 0.16 * a)),
            (1.0, ca(r, g, b, 0.0)),
        ]);
        self.circle(cx, cy, size / 2.0, sh);
    }

    /// Dải đồi: đường cong f(x) phủ xuống đáy.
    pub fn hill(&mut self, f: impl Fn(f32) -> f32, sh: Shader<'_>) {
        // vẽ dư ra để phủ cả khung dọc (720x1280) lẫn ngang (1280x720)
        let (w, h) = (W.max(H), W.max(H));
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, h);
        let mut x = 0.0;
        while x <= w {
            pb.line_to(x, f(x));
            x += 8.0;
        }
        pb.line_to(w, h);
        pb.close();
        if let Some(p) = pb.finish() {
            self.fill(&p, sh);
        }
    }
}

pub fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32, rot: f32) -> Option<Path> {
    let r = Rect::from_xywh(-rx.max(0.01), -ry.max(0.01), 2.0 * rx.max(0.01), 2.0 * ry.max(0.01))?;
    PathBuilder::from_oval(r)?.transform(Transform::from_translate(cx, cy).pre_rotate(rot.to_degrees()))
}

pub fn round_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.4477 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - k, y, x + w, y + k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - k, x + w - k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + k, y + h, x, y + h - k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + k, x + k, y, x + r, y);
    pb.close();
    pb.finish()
}

pub fn star_path(x: f32, y: f32, r1: f32, r2: f32, n: usize) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for i in 0..n * 2 {
        let r = if i % 2 == 1 { r2 } else { r1 };
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / n as f32;
        let (px, py) = (x + a.cos() * r, y + a.sin() * r);
        if i == 0 {
            pb.move_to(px, py)
        } else {
            pb.line_to(px, py)
        }
    }
    pb.close();
    pb.finish()
}

/// Cây thông nhiều tầng, đúng hình trong bản xem thử.
pub fn pine(cv: &mut Canvas, x: f32, y: f32, h: f32, col: Shader<'static>, snow: Option<Color>) {
    let w = h * 0.42;
    for i in 0..4 {
        let i = i as f32;
        let (ty, tw, th) = (y - h + i * h * 0.22, w * (0.45 + i * 0.2), h * 0.38);
        let mut pb = PathBuilder::new();
        pb.move_to(x, ty);
        pb.quad_to(x - tw * 0.4, ty + th * 0.6, x - tw, ty + th);
        pb.quad_to(x, ty + th * 0.86, x + tw, ty + th);
        pb.quad_to(x + tw * 0.4, ty + th * 0.6, x, ty);
        if let Some(p) = pb.finish() {
            cv.fill(&p, col.clone());
        }
        if let Some(sc) = snow {
            let mut pb = PathBuilder::new();
            pb.move_to(x, ty);
            pb.quad_to(x - tw * 0.25, ty + th * 0.35, x - tw * 0.55, ty + th * 0.52);
            pb.quad_to(x, ty + th * 0.36, x + tw * 0.55, ty + th * 0.52);
            pb.quad_to(x + tw * 0.25, ty + th * 0.35, x, ty);
            if let Some(p) = pb.finish() {
                cv.fill(&p, solid(sc));
            }
        }
    }
}

/// Atlas: gom các ảnh nhỏ (đèn lồng, dải cực quang...) vào một texture.
pub struct Atlas {
    pub pm: Pixmap,
    pub scale: f32,
    x: u32,
    y: u32,
    row_h: u32,
}

impl Atlas {
    pub fn new(scale: f32) -> Self {
        let size = if scale > 2.0 { 2048 } else { 1024 };
        Self { pm: Pixmap::new(size, size).expect("atlas"), scale, x: 1, y: 1, row_h: 0 }
    }

    /// Vẽ một ảnh kích thước thiết kế w x h; (ox, oy) là gốc toạ độ bên trong ảnh. Trả về uv.
    pub fn add(&mut self, w: f32, h: f32, ox: f32, oy: f32, draw: impl FnOnce(&mut Canvas)) -> [f32; 4] {
        let (pw, ph) = ((w * self.scale).ceil() as u32, (h * self.scale).ceil() as u32);
        let size = self.pm.width();
        if self.x + pw + 2 > size {
            self.x = 1;
            self.y += self.row_h + 2;
            self.row_h = 0;
        }
        let mut cv = Canvas::new(pw, ph, Transform::from_scale(self.scale, self.scale).pre_translate(ox, oy));
        draw(&mut cv);
        self.pm.draw_pixmap(self.x as i32, self.y as i32, cv.pm.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        let s = size as f32;
        let uv = [self.x as f32 / s, self.y as f32 / s, (self.x + pw) as f32 / s, (self.y + ph) as f32 / s];
        self.x += pw + 2;
        self.row_h = self.row_h.max(ph);
        uv
    }
}
