//! Giáng sinh: trời đêm, cực quang, trăng, đồi tuyết, rừng thông, nhà gỗ có khói,
//! cây thông lớn có đèn nháy, ngôi sao, quà, tuyết rơi ba lớp.

use tiny_skia::PathBuilder;

use super::Scene;
use crate::config::Config;
use crate::draw::*;
use crate::paint::*;

struct Star {
    x: f32,
    y: f32,
    r: f32,
    p: f32,
    s: f32,
}
struct Light {
    x: f32,
    y: f32,
    c: [f32; 3],
    p: f32,
}
struct Flake {
    x: f32,
    y: f32,
    z: usize,
    p: f32,
}

pub struct Noel {
    painted: bool,
    w: f32,
    h: f32,
    stars: Vec<Star>,
    lights: Vec<Light>,
    snow: Vec<Flake>,
    win: (f32, f32),
    chim: (f32, f32),
    star: (f32, f32),
    star_uv: [f32; 4],
    /// ông già Noel cưỡi xe tuần lộc bay ngang trời
    sleigh: Option<([f32; 4], f32, f32)>,
    rnd: Fast,
}

fn far_y(x: f32, lh: f32) -> f32 {
    lh * 0.63194 + (x * 0.0045 + 1.0).sin() * 26.0 + (x * 0.013 + 2.0).sin() * 11.0
}
fn mid_y(x: f32, lh: f32) -> f32 {
    lh * 0.72917 + (x * 0.0035 + 4.0).sin() * 28.0 + (x * 0.01).sin() * 12.0
}
fn near_y(x: f32, lh: f32) -> f32 {
    lh * 0.83889 + (x * 0.003 + 2.2).sin() * 20.0 + (x * 0.009 + 1.0).sin() * 8.0
}

impl Noel {
    pub fn build(back: &mut Canvas, front: &mut Canvas, atlas: &mut Atlas, l: super::Layout, painted: bool) -> Self {
        let (lw, lh, pt) = (l.w, l.h, l.portrait());
        let far = |x: f32| far_y(x, lh);
        let mid = |x: f32| mid_y(x, lh);
        let near = |x: f32| near_y(x, lh);
        let (mx, my) = (lw * 0.875, lh * 0.13889);
        let mut r = Rng::new(21);
        let g = back;
        g.rect(0.0, 0.0, lw, lh, lin(0.0, 0.0, 0.0, lh, &[(0.0, c(0x040a1a)), (0.45, c(0x0c1f3d)), (0.75, c(0x1d3a61)), (1.0, c(0x335882))]));
        g.glow(mx, my, 320.0, (190, 210, 255), 0.55);
        g.circle(mx, my, 30.0, rad(mx - 10.0, my - 10.0, mx, my, 30.0, &[(0.0, c(0xffffff)), (1.0, c(0xc9d6ec))]));
        for (x, y, rr) in [(-9.0, -6.0, 6.0), (9.0, 8.0, 4.5), (4.0, -13.0, 3.0)] {
            g.circle(mx + x, my + y, rr, solid(ca(110, 130, 170, 0.2)));
        }
        let mut stars = Vec::new();
        for _ in 0..170 {
            let (x, y, rr, p, s) = (r.f() * lw, r.f() * lh * 0.6, 0.6 + r.f() * 1.4, r.f() * TAU, 0.6 + r.f() * 2.0);
            stars.push(Star { x, y, r: rr, p, s });
        }
        let star_uv = atlas.add(36.0, 36.0, 18.0, 18.0, |cv| {
            if let Some(p) = star_path(0.0, 0.0, 16.0, 7.0, 5) {
                cv.fill(&p, rad(0.0, 0.0, 0.0, 0.0, 16.0, &[(0.12, c(0xfff6d0)), (1.0, c(0xf2b632))]));
            }
        });

        super::landscape::paint(super::Kind::Noel, g, l);
        let h = front;
        h.hill(far, lin(0.0, 420.0, 0.0, lh, &[(0.0, c(0x34557d)), (1.0, c(0x1d3355))]));
        for _ in 0..80 {
            let x = r.f() * lw;
            let th = 12.0 + r.f() * 22.0;
            pine(h, x, far(x) + 6.0, th, solid(c(0x172a45)), None);
        }
        h.hill(mid, lin(0.0, 490.0, 0.0, lh, &[(0.0, c(0xb4cbe4)), (1.0, c(0x7f9dc0))]));
        for _ in 0..32 {
            let x = r.f() * lw;
            if x > lw * 0.72656 && x < lw * 0.94531 {
                continue;
            }
            let th = 36.0 + r.f() * 48.0;
            pine(h, x, mid(x) + 8.0, th, solid(c(0x11302f)), Some(ca(214, 228, 243, 0.95)));
        }
        h.hill(near, lin(0.0, 570.0, 0.0, lh, &[(0.0, c(0xf4f9fe)), (1.0, c(0xc7d8ec))]));

        // Nhà gỗ
        let hx = lw * 0.234375;
        let gy = near(hx) + 6.0;
        h.rect(hx - 60.0, gy - 70.0, 120.0, 70.0, solid(c(0x3b2a2f)));
        for k in 0..6 {
            h.rect(hx - 60.0, gy - 70.0 + k as f32 * 12.0, 120.0, 1.5, solid(ca(0, 0, 0, 0.18)));
        }
        h.rect(hx + 28.0, gy - 134.0, 18.0, 40.0, solid(c(0x3b2a2f)));
        let mut pb = PathBuilder::new();
        pb.move_to(hx - 76.0, gy - 66.0);
        pb.line_to(hx, gy - 122.0);
        pb.line_to(hx + 76.0, gy - 66.0);
        pb.close();
        if let Some(p) = pb.finish() {
            h.fill(&p, solid(c(0x4a2c2a)));
        }
        let snowc = c(0xf6faff);
        let mut pb = PathBuilder::new();
        pb.move_to(hx - 84.0, gy - 62.0);
        pb.line_to(hx, gy - 128.0);
        pb.line_to(hx + 84.0, gy - 62.0);
        pb.quad_to(hx + 80.0, gy - 56.0, hx + 72.0, gy - 58.0);
        pb.line_to(hx, gy - 114.0);
        pb.line_to(hx - 72.0, gy - 58.0);
        pb.quad_to(hx - 80.0, gy - 56.0, hx - 84.0, gy - 62.0);
        if let Some(p) = pb.finish() {
            h.fill(&p, solid(snowc));
        }
        for (dx, ry) in [(-54.0, 7.0), (-30.0, 5.0), (20.0, 6.0), (50.0, 8.0)] {
            h.ellipse(hx + dx, gy - 60.0, 3.0, ry, 0.0, solid(snowc));
        }
        h.rect(hx + 26.0, gy - 140.0, 22.0, 7.0, solid(snowc));
        h.rect(hx - 44.0, gy - 54.0, 30.0, 26.0, solid(c(0xffcb6e)));
        h.rect(hx - 30.0, gy - 54.0, 2.5, 26.0, solid(c(0x3b2a2f)));
        h.rect(hx - 44.0, gy - 42.0, 30.0, 2.5, solid(c(0x3b2a2f)));
        h.rect(hx + 10.0, gy - 50.0, 26.0, 50.0, solid(c(0x5a3a2e)));
        h.circle(hx + 31.0, gy - 25.0, 2.0, solid(c(0xffcb6e)));
        h.ellipse(hx, gy + 2.0, 92.0, 10.0, 0.0, solid(c(0xeef5fc)));

        // Cây thông lớn, quà
        let (tx, th) = (lw * 0.8359375, if pt { 430.0 } else { 330.0 });
        let tb = near(tx) + 12.0;
        pine(h, tx, tb, th, lin(0.0, tb - th, 0.0, tb, &[(0.0, c(0x1b6a58)), (1.0, c(0x0a2f2a))]), Some(ca(236, 245, 253, 0.92)));
        h.rect(tx - 9.0, tb - 10.0, 18.0, 16.0, solid(c(0x3a2418)));
        h.ellipse(tx, tb + 6.0, 110.0, 12.0, 0.0, solid(c(0xeef5fc)));
        for (x, col, rb, s) in [(tx - 70.0, 0xe4534a, 0xffd27a, 30.0), (tx + 58.0, 0x2f9e7e, 0xf6faff, 26.0), (tx - 38.0, 0xf1c15b, 0xe4534a, 20.0)] {
            h.rect(x - s / 2.0, tb + 2.0 - s, s, s, solid(c(col)));
            h.rect(x - 2.0, tb + 2.0 - s, 4.0, s, solid(c(rb)));
            h.rect(x - s / 2.0, tb + 2.0 - s * 0.6, s, 4.0, solid(c(rb)));
        }
        let sleigh = tiny_skia::Pixmap::decode_png(include_bytes!("../../assets/foregrounds/noel-flight.png"))
            .ok()
            .map(|image| {
                let max_width: f32 = if pt { 139.0 } else { 184.0 };
                let scale = (max_width / image.width() as f32).min(48.0 / image.height() as f32);
                let (width, height) = (image.width() as f32 * scale, image.height() as f32 * scale);
                let uv = atlas.add(width, height, 0.0, 0.0, |cv| {
                    cv.pm.draw_pixmap(0, 0, image.as_ref(),
                        &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() },
                        cv.base.pre_scale(scale, scale), None);
                });
                (uv, width, height)
            });
        let cols = [rgb8(255, 196, 90), rgb8(255, 90, 80), rgb8(130, 220, 255), rgb8(255, 240, 205)];
        let mut lights = Vec::new();
        for i in 0..95 {
            let t = 0.08 + r.f() * 0.88;
            let y = tb - th + t * th * 0.96;
            let half = th * 0.42 * (0.18 + 0.9 * t) * 0.78;
            let x = tx + (r.f() * 2.0 - 1.0) * half;
            let (p, _speed) = (r.f() * TAU, r.f());
            lights.push(Light { x, y, c: cols[i % 4], p });
        }

        // 480 bông tuyết (240 đầu giống hệt bản xem thử); số bông hiện ra tuỳ cài đặt.
        let mut snow = Vec::new();
        for _ in 0..480 {
            let z = if r.f() < 0.55 { 0 } else if r.f() < 0.75 { 1 } else { 2 };
            let (x, y, p) = (r.f() * lw, r.f() * lh, r.f() * TAU);
            snow.push(Flake { x, y, z, p });
        }

        Noel {
            painted,
            w: lw,
            h: lh,
            stars,
            lights,
            snow,
            win: (hx - 29.0, gy - 41.0),
            chim: (hx + 37.0, gy - 142.0),
            star: (tx, tb - th + 2.0),
            sleigh,
            star_uv,
            rnd: Fast::seeded(),
        }
    }

    fn fly_sleigh(&self, t: f32, dl: &mut DrawList) {
        let Some((uv, width, height)) = self.sleigh else { return };
        let t = t.rem_euclid(33.0);
        const FLIGHT: f32 = 14.0;
        // Move towards the upper edge as the sleigh approaches the clock on the left.
        let sample = |time: f32| {
            let progress = (time / FLIGHT).clamp(0.0, 1.0);
            let x = self.w + width * 0.65 - (self.w + width * 1.85) * progress;
            let rise = (progress * 1.9).clamp(0.0, 1.0);
            let rise = rise * rise * (3.0 - 2.0 * rise);
            let scale = 1.0 - rise * 0.23;
            let ceiling = height * scale * 0.5 + 8.0;
            let y = (self.h * 0.167 * (1.0 - rise) + ceiling * rise)
                + (time * 1.7).sin() * 2.1;
            (x, y, scale)
        };
        for i in (1..=5).rev() {
            let age = i as f32 * 0.18;
            if t < age || t - age > FLIGHT {
                continue;
            }
            let (px, py, scale) = sample(t - age);
            let px = px + width * scale * 0.30;
            if px < -9.0 || px > self.w + 9.0 {
                continue;
            }
            let size = 9.0 - age * 5.0;
            dl.glow(Blend::Add, px, py + age * 3.0, size, size,
                rgb8(255, 223, 156), 0.34 * (1.0 - age));
        }
        if t >= FLIGHT {
            return;
        }
        let (x, y, scale) = sample(t);
        if x + width < 0.0 || x - width > self.w {
            return;
        }
        let (next_x, next_y, _) = sample(t + 0.03);
        let tilt = ((next_y - y) / (next_x - x).min(-0.1)).clamp(-0.11, 0.11);
        dl.atlas(uv, x, y, width * scale, height * scale, tilt, 0.93);
    }
}

impl Scene for Noel {
    fn frame(&mut self, t: f32, dt: f32, cfg: &Config, dl: &mut DrawList) {
        dl.layer(Tex::Back);
        for s in &self.stars {
            let a = 0.3 + 0.7 * (0.5 + 0.5 * s.p.sin()); // sao đứng yên
            let _ = s.s;
            dl.rect(Blend::Alpha, s.x + s.r / 2.0, s.y + s.r / 2.0, s.r, s.r, 0.0, [1.0; 3], a);
        }
        self.fly_sleigh(t, dl);
        if !self.painted {
            dl.layer(Tex::Front);
        }

        // Khói ống khói
        if !self.painted {
            // chỉ giữ tuyết rơi và xe tuần lộc: khói, đèn, sao đều đứng yên
            let sg = rgb8(205, 215, 230);
            for k in 0..5 {
                let a = k as f32 * 0.8;
                dl.glow(Blend::Alpha, self.chim.0 + a * 9.0, self.chim.1 - a * 15.0, (5.0 + a * 7.0) * 3.0, (5.0 + a * 7.0) * 3.0, sg, 0.22 * (1.0 - a / 4.0));
            }

            // Cửa sổ sáng, đèn cây thông, ngôi sao
            let wa = 0.5;
            dl.glow(Blend::Add, self.win.0, self.win.1, 160.0, 160.0, rgb8(255, 180, 80), wa);
            for l in &self.lights {
                let a = 0.5 + 0.5 * l.p.sin();
                dl.glow(Blend::Add, l.x, l.y, 20.0, 20.0, l.c, 0.35 + 0.65 * a);
            }
            dl.glow(Blend::Add, self.star.0, self.star.1, 90.0, 90.0, rgb8(255, 215, 120), 0.8);
            for l in &self.lights {
                dl.disc(Blend::Alpha, l.x, l.y, 1.8, 1.8, 0.0, l.c, 1.0);
            }
            dl.atlas(self.star_uv, self.star.0, self.star.1, 36.0, 36.0, 0.0, 1.0);
        }

        // Tuyết
        let n = ((240.0 * cfg.snow) as usize).min(self.snow.len());
        let (sz, sp, sw) = ([1.3, 2.3, 4.0], [16.0, 30.0, 52.0], [6.0, 12.0, 20.0]);
        for s in self.snow.iter_mut().take(n) {
            s.y += sp[s.z] * dt;
            s.x += ((t * 0.8 + s.p).sin() * sw[s.z] * 0.5 + 6.0) * dt;
            if s.y > self.h + 6.0 {
                s.y = -6.0;
                s.x = self.rnd.f() * self.w;
            }
            if s.x > self.w + 6.0 {
                s.x = -6.0;
            }
            if s.z < 2 {
                let r = sz[s.z] / 2.0 + 0.3;
                dl.disc(Blend::Alpha, s.x, s.y, r, r, 0.0, [1.0; 3], if s.z == 1 { 0.85 } else { 0.55 });
            } else {
                dl.glow(Blend::Alpha, s.x, s.y, 15.0, 15.0, [1.0; 3], 0.8);
            }
        }
    }
}
