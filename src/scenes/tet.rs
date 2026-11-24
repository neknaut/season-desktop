//! Tết Nguyên Đán: mặt trời vàng, mây tường vân, cành mai, đèn lồng đung đưa,
//! pháo hoa, đốm sáng bay lên, cánh mai rơi.

use std::f32::consts::{FRAC_PI_2, PI};
use tiny_skia::{PathBuilder, Transform};

use super::{load_art, Scene};
use crate::config::Config;
use crate::draw::*;
use crate::paint::*;

struct Lantern {
    x: f32,
    len: f32,
    r: f32,
    p: f32,
}
struct Bokeh {
    x: f32,
    y: f32,
    s: f32,
    a: f32,
}
struct Petal {
    x: f32,
    y: f32,
    v: f32,
    rot: f32,
    vr: f32,
    flip: f32,
    vf: f32,
    s: f32,
    palette: usize,
    p: f32,
}
struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    c: [f32; 3],
}
struct Bloom {
    x: f32,
    y: f32,
    s: f32,
    r: f32,
    bud: bool,
}

/// Gia đình nấu bánh chưng và múa lân.
struct Village {
    /// bếp lửa: (x, mặt đất, cỡ nồi)
    pot: (f32, f32, f32),
    /// múa lân: (x, mặt đất)
    lion: (f32, f32),
    lion_uv: [f32; 4],
    cloth_uv: [f32; 4],
    flame_uv: [f32; 4],
    /// ảnh lân riêng của người dùng (art	et\lan.png): uv, rộng, cao (đơn vị thiết kế)
    lion_img: Option<([f32; 4], f32, f32)>,
    /// khói bốc lên từ nồi: x, y, tuổi
    steam: Vec<(f32, f32, f32)>,
    steam_t: f32,
}

/// Kích thước ảnh trong atlas (đơn vị thiết kế).
const LION_S: f32 = 46.0;

pub struct Tet {
    painted: bool,
    w: f32,
    h: f32,
    village: Village,
    lanterns: Vec<Lantern>,
    bokeh: Vec<Bokeh>,
    petals: Vec<Petal>,
    petal_uv: [[f32; 4]; 2],
    sparks: Vec<Spark>,
    fw_t: f32,
    lantern_uv: [f32; 4],
    rnd: Fast,
}

/// Ảnh đèn lồng vẽ ở r = 34, gốc (0, 0) là đầu dây chỗ chóp đèn.
const LR: f32 = 34.0;
const LW: f32 = 72.0;
const LH: f32 = 14.0 + 3.35 * LR + 2.0;

fn cloud(g: &mut Canvas, x: f32, y: f32, s: f32, fill: tiny_skia::Color, stroke: tiny_skia::Color) {
    let cs = [(0.0, 0.0, 1.0), (0.95, -0.35, 0.8), (1.75, 0.05, 0.85), (-0.95, 0.08, 0.72), (0.55, 0.38, 0.7), (1.3, 0.4, 0.6)];
    for (cx, cy, cr) in cs {
        g.circle(x + cx * s, y + cy * s, cr * s, solid(fill));
    }
    for &(cx, cy, cr) in &cs[..4] {
        for (k, a0, a1) in [(0.62, 1.05, 1.9), (0.3, 1.1, 1.85)] {
            let (ox, oy, rr) = (x + cx * s, y + cy * s, cr * s * k);
            let mut pb = PathBuilder::new();
            let steps = 16;
            for i in 0..=steps {
                let a = PI * (a0 + (a1 - a0) * i as f32 / steps as f32);
                let (px, py) = (ox + a.cos() * rr, oy + a.sin() * rr);
                if i == 0 {
                    pb.move_to(px, py)
                } else {
                    pb.line_to(px, py)
                }
            }
            if let Some(p) = pb.finish() {
                g.stroke(&p, solid(stroke), 2.0);
            }
        }
    }
}

fn flower(g: &mut Canvas, b: &Bloom) {
    let s = b.s;
    let t = Transform::from_translate(b.x, b.y).pre_rotate(b.r.to_degrees());
    if b.bud {
        if let Some(p) = ellipse_path(0.0, 0.0, s * 0.36, s * 0.48, 0.0) {
            g.fill_t(&p, rad_t(-1.0, -1.0, 0.0, 0.0, s * 0.55, &[(0.0, c(0xfff1a8)), (1.0, c(0xd99a12))], Transform::identity()), t);
        }
        if let Some(p) = ellipse_path(0.0, s * 0.42, s * 0.26, s * 0.16, 0.0) {
            g.fill_t(&p, solid(c(0x7a6a24)), t);
        }
        return;
    }
    let petal = rad_t(0.0, 0.0, 0.0, 0.0, s * 1.08, &[(0.09, c(0xfff6c4)), (0.5, c(0xffd84a)), (1.0, c(0xef9f12))], Transform::identity());
    for i in 0..5 {
        let a = i as f32 * TAU / 5.0;
        if let Some(p) = ellipse_path(a.cos() * s * 0.52, a.sin() * s * 0.52, s * 0.56, s * 0.43, a) {
            g.fill_t(&p, petal.clone(), t);
        }
    }
    for i in 0..9 {
        let a = i as f32 * TAU / 9.0;
        let (ex, ey) = (a.cos() * s * 0.48, a.sin() * s * 0.48);
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0);
        pb.line_to(ex, ey);
        if let Some(p) = pb.finish().and_then(|p| p.transform(t)) {
            g.stroke(&p, solid(c(0xc2541a)), 0.7);
        }
        if let Some(p) = PathBuilder::from_rect(tiny_skia::Rect::from_xywh(ex - 0.8, ey - 0.8, 1.6, 1.6).unwrap()).transform(t) {
            g.fill(&p, solid(c(0xfff0a0)));
        }
    }
    if let Some(p) = PathBuilder::from_circle(0.0, 0.0, s * 0.17) {
        g.fill_t(&p, solid(c(0xd9661c)), t);
    }
}

#[allow(clippy::too_many_arguments)]
fn seg(g: &mut Canvas, r: &mut Rng, bl: &mut Vec<Bloom>, x: f32, y: f32, a: f32, len: f32, w: f32, d: i32) {
    let (ex, ey) = (x + a.cos() * len, y + a.sin() * len);
    let bend = (r.f() - 0.5) * len * 0.35;
    let mx = (x + ex) / 2.0 + (a + FRAC_PI_2).cos() * bend;
    let my = (y + ey) / 2.0 + (a + FRAC_PI_2).sin() * bend;
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.quad_to(mx, my, ex, ey);
    if let Some(p) = pb.finish() {
        g.stroke(&p, solid(c(0x2e140c)), w);
    }
    if w > 3.0 {
        let o = w * 0.22;
        let mut pb = PathBuilder::new();
        pb.move_to(x - o, y - o);
        pb.quad_to(mx - o, my - o, ex - o, ey - o);
        if let Some(p) = pb.finish() {
            g.stroke(&p, solid(ca(150, 88, 55, 0.5)), w * 0.28);
        }
    }
    if d <= 3 {
        let n = 1 + (r.f() * 3.0).floor() as i32;
        for _ in 0..n {
            let t = 0.3 + r.f() * 0.7;
            let u = 1.0 - t;
            let px = u * u * x + 2.0 * u * t * mx + t * t * ex;
            let py = u * u * y + 2.0 * u * t * my + t * t * ey;
            let off = (r.f() - 0.5) * (w + 12.0);
            let (s, rr, bud) = (6.0 + r.f() * 6.5, r.f() * TAU, r.f() < 0.18);
            bl.push(Bloom { x: px + (a + FRAC_PI_2).cos() * off, y: py + (a + FRAC_PI_2).sin() * off, s, r: rr, bud });
        }
    }
    if d == 0 {
        for _ in 0..3 {
            let (bx, by) = (ex + (r.f() - 0.5) * 16.0, ey + (r.f() - 0.5) * 16.0);
            let (s, rr, bud) = (5.0 + r.f() * 6.0, r.f() * TAU, r.f() < 0.35);
            bl.push(Bloom { x: bx, y: by, s, r: rr, bud });
        }
        return;
    }
    let kids = if d > 4 { 2 } else if r.f() < 0.55 { 2 } else { 3 };
    for i in 0..kids {
        let na = if i == 0 {
            a + (r.f() - 0.5) * 0.35
        } else {
            let sign = if r.f() < 0.5 { -1.0 } else { 1.0 };
            a + sign * (0.35 + r.f() * 0.5)
        };
        let nl = len * (0.68 + r.f() * 0.12);
        seg(g, r, bl, ex, ey, na, nl, w * 0.62, d - 1);
    }
}

fn draw_lantern(g: &mut Canvas) {
    let r = LR;
    let cy = 8.0 + r * 1.1;
    g.round_rect(-r * 0.5, 0.0, r, 9.0, 3.0, solid(c(0xe8b64c)));
    g.ellipse(0.0, cy, r, r * 1.1, 0.0, rad(-r * 0.25, cy - r * 0.3, 0.0, cy, r * 1.25, &[(0.08, c(0xffc27a)), (0.38, c(0xf0482f)), (1.0, c(0x8f0d17))]));
    let rib = ca(255, 205, 110, 0.5);
    for k in [0.35, 0.7] {
        if let Some(p) = ellipse_path(0.0, cy, r * k, r * 1.1, 0.0) {
            g.stroke(&p, solid(rib), 1.0);
        }
    }
    g.line(0.0, cy - r * 1.1, 0.0, cy + r * 1.1, solid(rib), 1.0);
    g.round_rect(-r * 0.5, 0.0, r, 9.0, 3.0, solid(c(0xe8b64c)));
    g.round_rect(-r * 0.42, cy + r * 1.1 - 3.0, r * 0.84, 8.0, 3.0, solid(c(0xe8b64c)));
    for i in -2..=2 {
        let i = i as f32;
        g.line(i * 1.6, cy + r * 1.1 + 5.0, i * 2.4, cy + r * 1.1 + 5.0 + r * 1.15, solid(c(0xe8b64c)), 1.2);
    }
}

impl Tet {
    pub fn build(back: &mut Canvas, front: &mut Canvas, atlas: &mut Atlas, l: super::Layout, painted: bool) -> Self {
        let (lw, lh, pt) = (l.w, l.h, l.portrait());
        // màn dọc: trăng cao hơn để chừa chỗ cho đồng hồ giữa trăng và cảnh gia đình
        let (sx, sy) = if pt { (lw * 0.56, lh * 0.38) } else { (lw * 0.546875, lh * 0.47222) };
        let mut r = Rng::new(8);
        let g = back;
        g.rect(0.0, 0.0, lw, lh, lin(0.0, 0.0, 0.0, lh, &[(0.0, c(0x24050c)), (0.45, c(0x5e0c16)), (0.8, c(0x9a2420)), (1.0, c(0xc9532a))]));
        g.glow(sx, sy, 860.0, (255, 170, 80), 0.6);
        for rr in [200.0, 232.0, 268.0] {
            if let Some(p) = PathBuilder::from_circle(sx, sy, rr) {
                g.stroke(&p, solid(ca(255, 210, 130, 0.16)), 1.5);
            }
        }
        g.circle(sx, sy, 165.0, rad(sx - 50.0, sy - 50.0, sx, sy, 165.0, &[(0.06, c(0xffe6a3)), (0.55, c(0xffb547)), (1.0, c(0xf07b24))]));
        cloud(g, lw * if pt { 0.8 } else { 0.4375 }, lh * if pt { 0.53 } else { 0.63194 }, 46.0, c(0xa83226), ca(255, 214, 140, 0.6));
        cloud(g, lw * if pt { 0.86 } else { 0.6875 }, lh * if pt { 0.66 } else { 0.70139 }, 40.0, c(0xa1302a), ca(255, 214, 140, 0.55));
        let (c3x, c3y) = if pt { (lw * 0.16, lh * 0.36) } else { (lw * 0.22656, lh * 0.59722) };
        cloud(g, c3x, c3y, 32.0, c(0x7d1a1f), ca(255, 214, 140, 0.4));
        cloud(g, lw * 0.78906, lh * 0.22222, 28.0, c(0x6e141b), ca(255, 214, 140, 0.35));
        super::landscape::paint(super::Kind::Tet, g, l);
        g.hill(|x| lh * 0.83333 + (x * 0.004).sin() * 18.0 + (x * 0.011 + 1.0).sin() * 8.0, solid(c(0x4d0a14)));
        g.hill(|x| lh * 0.89167 + (x * 0.006 + 2.0).sin() * 12.0, solid(c(0x330610)));

        let mut bl = Vec::new();
        if pt {
            // khung dọc: cành mai thấp hơn, ngắn hơn để không đè lên đèn lồng
            seg(front, &mut r, &mut bl, lw + 30.0, lh * 0.05, 2.55, 120.0, 16.0, 6);
        } else {
            seg(front, &mut r, &mut bl, lw + 30.0, -20.0, 2.25, 210.0, 22.0, 6);
        }
        seg(front, &mut r, &mut bl, lw + 20.0, lh * if pt { 0.30 } else { 0.36111 }, 2.95, if pt { 120.0 } else { 150.0 }, 12.0, 5);
        for b in &bl {
            flower(front, b);
        }

        let lantern_uv = atlas.add(LW, LH, LW / 2.0, 1.0, draw_lantern);

        // ---- gia đình quây quần gói và nấu bánh chưng (kiểu tranh minh hoạ)
        use super::figures::*;
        let gy = |x: f32| lh * 0.89167 + (x * 0.006 + 2.0).sin() * 12.0 + 6.0;
        let sticker = tiny_skia::Pixmap::decode_png(include_bytes!("../../assets/foregrounds/tet.png")).ok();
        let uses_sticker = sticker.is_some();
        let family_img = sticker.or_else(|| load_art("tet", "giadinh.png"));
        let draw_pot = uses_sticker || family_img.is_none();
        let px = lw * if uses_sticker { if pt { 0.59 } else { 0.66 } } else if pt { 0.36 } else { 0.40 };
        let gp = gy(px);
        let ps = if uses_sticker && pt { 31.0 } else { 40.0 };
        front.glow(px, gp - 60.0, 460.0, (255, 160, 70), 0.5);
        if let Some(img) = &family_img {
            let max_h = if pt { 185.0 } else { 182.0 };
            let max_w = lw * if uses_sticker { if pt { 0.45 } else { 0.44 } } else { 0.72 };
            let k = (max_h / img.height() as f32).min(max_w / img.width() as f32);
            let (ww, hh) = (img.width() as f32 * k, img.height() as f32 * k);
            let cx = if uses_sticker { lw * if pt { 0.30 } else { 0.435 } } else { lw * 0.40 };
            let t = front.base.pre_translate(cx - ww / 2.0, gy(cx) + 10.0 - hh).pre_scale(k, k);
            front.pm.draw_pixmap(0, 0, img.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, t, None);
            if uses_sticker {
                pot_art(front, px, gp, ps);
            }
        } else {
        // chiếu trải dưới đất
        front.ellipse(px + 30.0, gp + 2.0, 230.0, 16.0, 0.0, solid(c(0xe9c46a)));
        for k in 0..10 {
            front.line(px - 190.0 + k as f32 * 42.0, gp - 12.0, px - 175.0 + k as f32 * 42.0, gp + 16.0, solid(ca(160, 110, 40, 0.35)), 1.5);
        }
        let o = |shirt: u32, pants: u32, hair: Hair, hc: u32, band: Option<u32>| Outfit { shirt, pants, hair, hair_col: hc, band, aodai: pants == 0xf1f3f5, pattern: 0xffd6e7 };
        // bà (ngồi, tóc bạc búi)
        chibi(front, px - 175.0, gp, 100.0, Pose::Sit, 1.0, o(0x8d5a3a, 0x3b2a20, Hair::Bun, 0xc9c9c9, None));
        // bố trông lửa
        chibi(front, px - 92.0, gp, 112.0, Pose::Squat, 1.0, o(0x1c7ed6, 0x343a40, Hair::Short, 0x2b2b2b, None));
        pot_art(front, px, gp, ps);
        // mẹ áo dài tím quỳ gói bánh
        let mh = chibi(front, px + 98.0, gp, 112.0, Pose::Sit, -1.0, o(0x9775fa, 0xf1f3f5, Hair::Long, 0x2b2b2b, Some(0xb197fc)));
        front.rect(mh.0 - 26.0, gp - 6.0, 34.0, 6.0, solid(c(0x2f9e44)));
        banh_chung(front, mh.0 - 10.0, gp - 6.0, 16.0);
        for (dx, dy) in [(0.0f32, 0.0f32), (22.0, 0.0), (11.0, -20.0)] {
            banh_chung(front, px + 150.0 + dx, gp + dy, 20.0);
        }
        // các con: bé trai áo vàng khăn vàng, bé gái áo hồng tóc hai chỏm
        chibi(front, px + 200.0, gp, 80.0, Pose::Stand, -1.0, o(0xffd43b, 0xf1f3f5, Hair::Short, 0x2b2b2b, Some(0xfab005)));
        chibi(front, px + 248.0, gp, 74.0, Pose::Cheer, -1.0, o(0xf783ac, 0xc2255c, Hair::Pigtails, 0x2b2b2b, None));

        }
        // ---- múa lân (kiểu tranh minh hoạ)
        let lx = lw * if family_img.is_some() { if pt { 0.72 } else { 0.785 } } else if pt { 0.68 } else { 0.72 };
        let lion = (lx, gy(lx));
        let lion_uv = atlas.add(LION_S * 2.6, LION_S * 2.7, LION_S * 1.3, LION_S * 1.5, |cv| lion_head_art(cv, LION_S));
        let cloth_uv = atlas.add(LION_S * 2.95, LION_S * 2.05, LION_S * 0.05, LION_S * 1.02, |cv| lion_body_art(cv, LION_S));
        let flame_uv = atlas.add(16.0, 28.0, 8.0, 20.0, |cv| {
            let mut pb = PathBuilder::new();
            pb.move_to(0.0, -19.0);
            pb.quad_to(8.0, -4.0, 6.0, 4.0);
            pb.quad_to(0.0, 10.0, -6.0, 4.0);
            pb.quad_to(-8.0, -4.0, 0.0, -19.0);
            if let Some(p) = pb.finish() {
                cv.fill(&p, lin(0.0, -19.0, 0.0, 8.0, &[(0.0, c(0xfff3a0)), (0.5, c(0xffa53c)), (1.0, c(0xe8401c))]));
            }
        });
        // Ảnh lân riêng (PNG nền trong suốt) nếu người dùng đặt vào thư mục art	et: dùng thay hình vẽ
        let lion_img = load_art("tet", "lan.png").map(|png| {
            // 240 đơn vị thiết kế ≈ 360 px trên màn 1920: ảnh 360 px hiện đúng cỡ gốc, không nhoè
            let hh = if pt { 178.0 } else { 240.0 };
            let k = hh / png.height().max(1) as f32;
            let ww = png.width() as f32 * k;
            let uv = atlas.add(ww, hh, 0.0, 0.0, |cv| {
                let t = cv.base.pre_scale(k, k);
                cv.pm.draw_pixmap(0, 0, png.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, t, None);
            });
            (uv, ww, hh)
        });
        let ps = if draw_pot { ps } else { 0.0 };
        let village = Village { pot: (px, gp, ps), lion, lion_uv, cloth_uv, flame_uv, lion_img, steam: Vec::new(), steam_t: 0.0 };

        let mut bokeh = Vec::new();
        for _ in 0..22 {
            // (giữ đủ các lần lấy số ngẫu nhiên để những thứ dựng sau vẫn ở đúng chỗ)
            let (x, y, s, _speed, _phase, a) = (r.f() * lw, r.f() * lh, 14.0 + r.f() * 40.0, r.f(), r.f(), 0.08 + r.f() * 0.16);
            bokeh.push(Bokeh { x, y, s, a });
        }
        let petal_uv = [0xf4be57, 0xffd889].map(|color| {
            super::particles::stamp(atlas, 22.0, color, super::particles::Shape::Apricot)
        });
        let mut petals = Vec::new();
        for _ in 0..46 {
            let (x, y, v, rot, vr, flip, vf, s) = (lw * 0.390625 + r.f() * lw * 0.640625, r.f() * lh, 18.0 + r.f() * 26.0, r.f() * TAU, (r.f() - 0.5) * 3.0, r.f() * TAU, 1.0 + r.f() * 3.0, 3.0 + r.f() * 3.0);
            let palette = usize::from(r.f() >= 0.5);
            let p = r.f() * TAU;
            petals.push(Petal { x, y, v, rot, vr, flip, vf, s, palette, p });
        }
        Tet {
            painted,
            w: lw,
            h: lh,
            village,
            lanterns: if painted { Vec::new() } else { vec![
                Lantern { x: 78.0, len: 70.0, r: 34.0, p: 0.0 },
                Lantern { x: 162.0, len: 135.0, r: 27.0, p: 1.7 },
                Lantern { x: 232.0, len: 40.0, r: 22.0, p: 3.1 },
            ] },
            bokeh,
            petals,
            petal_uv,
            sparks: Vec::new(),
            fw_t: 0.2,
            lantern_uv,
            rnd: Fast::seeded(),
        }
    }
}

impl Tet {
    /// Lửa bếp, khói nồi bánh chưng và con lân múa (vẽ sau lớp tiền cảnh).
    fn draw_village(&mut self, t: f32, dt: f32, dl: &mut DrawList) {
        let v = &mut self.village;
        let (px, gp, ps) = v.pot;
        if ps <= 0.0 {
            // dùng ảnh gia đình riêng: không vẽ lửa/khói chồng lên ảnh
            return self.draw_lion(t, dl);
        }
        // lửa bập bùng
        let flick = 0.75 + 0.25 * (t * 13.0).sin() * (t * 7.3).cos();
        dl.glow(Blend::Add, px, gp - ps * 0.35, ps * 4.0 * flick, ps * 3.0 * flick, rgb8(255, 140, 50), 0.45);
        for (k, dx) in [-0.35f32, 0.0, 0.32].iter().enumerate() {
            let f = 0.7 + 0.3 * (t * (9.0 + k as f32 * 2.3) + k as f32).sin();
            dl.atlas(v.flame_uv, px + dx * ps, gp - ps * 0.22 - 6.0 * f, 16.0 * f * 1.2, 28.0 * f * 1.3, (t * 5.0 + k as f32).sin() * 0.12, 0.95);
        }
        // khói nồi
        v.steam_t -= dt;
        if v.steam_t <= 0.0 {
            v.steam_t = 0.35;
            let top = gp - ps * 0.5 - ps * 0.95;
            v.steam.push((px + (t * 3.1).sin() * ps * 0.3, top - 4.0, 0.0));
        }
        for s in &mut v.steam {
            s.2 += dt;
            s.1 -= 16.0 * dt;
            s.0 += (t * 0.8 + s.2).sin() * 6.0 * dt;
            let life = (s.2 / 4.0).min(1.0);
            let size = 18.0 + life * 60.0;
            dl.glow(Blend::Alpha, s.0, s.1, size, size, rgb8(255, 236, 220), 0.32 * (1.0 - life));
        }
        v.steam.retain(|s| s.2 < 4.0);

        self.draw_lion(t, dl);
    }
}

impl Tet {
    fn draw_lion(&mut self, t: f32, dl: &mut DrawList) {
        let v = &mut self.village;
        // múa lân: đầu lắc lư, nhún nhảy; thân nhún theo nhịp, hơi trễ hơn đầu
        let (lx, lg) = v.lion;
        if let Some((uv, ww, hh)) = v.lion_img {
            // ảnh lân của người dùng: nhún nhảy, lắc lư cả con
            let sway = (t * 0.6).sin() * 24.0;
            let bob = (t * 3.2).sin().abs() * 10.0;
            dl.glow(Blend::Add, lx + sway, lg - hh * 0.5, ww * 1.6, hh * 1.3, rgb8(255, 120, 60), 0.14);
            // ảnh thường có khoảng trống quanh viền: hạ xuống một chút cho chân chạm đất
            dl.atlas(uv, lx + sway, lg - hh * 0.42 - bob, ww, hh, (t * 1.7).sin() * 0.06, 1.0);
            return;
        }
        let sway = (t * 0.6).sin() * 24.0;
        let bob = (t * 3.2).sin().abs() * 9.0;
        let body_bob = (t * 3.2 - 0.6).sin().abs() * 6.0;
        let (hx, hy) = (lx + sway, lg - LION_S * 2.05 - bob);
        dl.glow(Blend::Add, hx + LION_S, hy, LION_S * 6.0, LION_S * 4.0, rgb8(255, 120, 60), 0.16);
        dl.atlas(v.cloth_uv, hx + LION_S * 0.55 + LION_S * 1.42, lg - LION_S * 0.98 - body_bob, LION_S * 2.95, LION_S * 2.05, (t * 3.2 - 0.6).cos() * 0.03, 1.0);
        let tilt = (t * 1.7).sin() * 0.13 - bob * 0.006;
        dl.atlas(v.lion_uv, hx, hy + LION_S * 0.15, LION_S * 2.6, LION_S * 2.7, tilt, 1.0);
    }
}

impl Scene for Tet {
    fn clock_position(&self, kind: super::Kind, layout: super::Layout) -> (f32, f32) {
        crate::widget::position(kind, layout, self.painted)
    }
    fn frame(&mut self, t: f32, dt: f32, cfg: &Config, dl: &mut DrawList) {
        dl.layer(Tex::Back);
        let gb = rgb8(255, 200, 110);
        // Tết chỉ giữ hoa mai rơi: đốm sáng, pháo hoa, đèn lồng, khói đều đứng yên hoặc bỏ
        for k in &self.bokeh {
            dl.glow(Blend::Add, k.x, k.y, k.s, k.s, gb, k.a * 0.7);
        }

        if false && cfg.fireworks {
            self.fw_t -= dt;
            if self.fw_t <= 0.0 {
                self.fw_t = 1.1 + self.rnd.f() * 1.5;
                let (x, y) = (self.w * 0.3359 + self.rnd.f() * self.w * 0.328, 80.0 + self.rnd.f() * 140.0);
                let cols = [rgb8(255, 214, 120), rgb8(255, 120, 90), rgb8(255, 240, 210), rgb8(255, 170, 90)];
                let col = cols[(self.rnd.f() * 4.0) as usize % 4];
                let n = 64;
                for i in 0..n {
                    let a = i as f32 / n as f32 * TAU + self.rnd.f() * 0.05;
                    let sp = 85.0 + self.rnd.f() * 65.0;
                    self.sparks.push(Spark { x, y, vx: a.cos() * sp, vy: a.sin() * sp, age: 0.0, life: 1.4 + self.rnd.f() * 0.7, c: col });
                }
            }
        }
        let damp = 0.3f32.powf(dt);
        for q in &mut self.sparks {
            q.age += dt;
            q.vx *= damp;
            q.vy *= damp;
            q.vy += 38.0 * dt;
            q.x += q.vx * dt;
            q.y += q.vy * dt;
            let a = (1.0 - q.age / q.life).max(0.0);
            let len = (q.vx * q.vx + q.vy * q.vy).sqrt() * 0.1;
            dl.rect(Blend::Add, q.x - q.vx * 0.05, q.y - q.vy * 0.05, len + 1.8, 1.8, q.vy.atan2(q.vx), q.c, a);
            dl.glow(Blend::Add, q.x, q.y, 9.0, 9.0, q.c, a);
        }
        self.sparks.retain(|q| q.age < q.life);

        if !self.painted {
            dl.layer(Tex::Front);
        }
        if !self.painted {
            self.draw_village(0.0, 0.0, dl);
        } else if false {
            // Coordinates follow the painted cooking pot in each composition.
            let (x, y) = if self.w < self.h { (0.13, 0.52) } else { (0.105, 0.545) };
            for i in 0..5 {
                let age = (t * 0.29 + i as f32 / 5.0).fract();
                let sx = x * self.w + age * 16.0 + (age * 5.3 + t * 0.6).sin() * 7.0;
                let sy = y * self.h - age * 92.0;
                let size = 20.0 + 42.0 * age;
                let opacity = 0.16 * (std::f32::consts::PI * age).sin();
                dl.glow(Blend::Alpha, sx, sy, size, size * 0.78, rgb8(255, 223, 179), opacity);
            }
        }

        for l in &self.lanterns {
            let ang = 0.0f32; // đèn lồng đứng yên
            let _ = l.p;
            let (sn, cs) = ang.sin_cos();
            // điểm (0, d) trong hệ toạ độ đèn sau khi xoay quanh móc treo
            let at = |d: f32| (l.x - sn * d, cs * d);
            let (sx, sy) = at(l.len / 2.0);
            dl.rect(Blend::Alpha, sx, sy, 1.4, l.len, ang, hex(0xe0b04f), 1.0);
            let k = l.r / LR;
            let (gx, gy) = at(l.len + 8.0 + l.r * 1.1);
            dl.glow(Blend::Add, gx, gy, l.r * 6.8, l.r * 6.8, rgb8(255, 90, 50), 0.55);
            let (bx, by) = at(l.len + (LH / 2.0 - 1.0) * k);
            dl.atlas(self.lantern_uv, bx, by, LW * k, LH * k, ang, 1.0);
        }

        if cfg.petals {
            for p in &mut self.petals {
                p.y += p.v * dt;
                p.x += (-14.0 + (t * 0.7 + p.p).sin() * 10.0) * dt;
                p.rot += p.vr * dt;
                p.flip += p.vf * dt;
                if p.y > self.h + 10.0 || p.x < -10.0 {
                    p.y = -10.0 - self.rnd.f() * 60.0;
                    p.x = self.w * 0.46875 + self.rnd.f() * self.w * 0.546875;
                }
                let flutter = p.flip.cos().abs();
                let size = p.s * 2.9;
                let edge = (p.y / 28.0).min((self.h - p.y) / 42.0).clamp(0.0, 1.0);
                dl.atlas(self.petal_uv[p.palette], p.x, p.y,
                    size, size * (0.25 + 0.75 * flutter), p.rot, edge * (0.63 + 0.25 * flutter));
            }
        }
    }
}
