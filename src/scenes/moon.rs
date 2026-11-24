//! Trung thu: trăng rằm có bóng cây đa, mây trôi, đình làng và cây đa in bóng,
//! mặt hồ lấp lánh ánh trăng, đèn trời bay lên.

use std::f32::consts::PI;
use tiny_skia::{PathBuilder, Transform};

use super::Scene;
use crate::config::Config;
use crate::draw::*;
use crate::paint::*;

struct Cloud {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    v: f32,
}
struct SkyLantern {
    x: f32,
    y: f32,
    z: f32,
    p: f32,
}

pub struct Moon {
    painted: bool,
    w: f32,
    h: f32,
    mx: f32,
    wy: f32,
    clouds: Vec<Cloud>,
    sky: Vec<SkyLantern>,
    sky_uv: [f32; 4],
    /// trẻ em rước đèn ông sao: đầu cán đèn (x, y) và ảnh đèn
    kids: Vec<(f32, f32, f32)>,
    star_uv: [f32; 4],
    rnd: Fast,
}

/// Đèn trời vẽ ở kích thước s = 26, tâm (0, 0).
const SL: f32 = 26.0;

fn pavilion(g: &mut Canvas, px: f32, py: f32, col: tiny_skia::Color, t: Transform) {
    let mut shapes: Vec<tiny_skia::Path> = Vec::new();
    let rect = |x: f32, y: f32, w: f32, h: f32| PathBuilder::from_rect(tiny_skia::Rect::from_xywh(x, y, w, h).unwrap());
    shapes.push(rect(px - 130.0, py - 14.0, 260.0, 14.0));
    for k in [-100.0, -50.0, 0.0, 50.0, 100.0] {
        shapes.push(rect(px + k - 4.0, py - 82.0, 8.0, 70.0));
    }
    let mut pb = PathBuilder::new();
    pb.move_to(px - 168.0, py - 104.0);
    pb.quad_to(px - 150.0, py - 82.0, px - 110.0, py - 84.0);
    pb.line_to(px + 110.0, py - 84.0);
    pb.quad_to(px + 150.0, py - 82.0, px + 168.0, py - 104.0);
    pb.quad_to(px + 120.0, py - 110.0, px + 90.0, py - 140.0);
    pb.line_to(px - 90.0, py - 140.0);
    pb.quad_to(px - 120.0, py - 110.0, px - 168.0, py - 104.0);
    if let Some(p) = pb.finish() {
        shapes.push(p);
    }
    shapes.push(rect(px - 62.0, py - 160.0, 124.0, 22.0));
    let mut pb = PathBuilder::new();
    pb.move_to(px - 118.0, py - 176.0);
    pb.quad_to(px - 104.0, py - 158.0, px - 76.0, py - 160.0);
    pb.line_to(px + 76.0, py - 160.0);
    pb.quad_to(px + 104.0, py - 158.0, px + 118.0, py - 176.0);
    pb.quad_to(px + 84.0, py - 180.0, px + 60.0, py - 202.0);
    pb.line_to(px - 60.0, py - 202.0);
    pb.quad_to(px - 84.0, py - 180.0, px - 118.0, py - 176.0);
    if let Some(p) = pb.finish() {
        shapes.push(p);
    }
    shapes.push(rect(px - 3.0, py - 218.0, 6.0, 18.0));
    if let Some(p) = PathBuilder::from_circle(px, py - 220.0, 6.0) {
        shapes.push(p);
    }
    for s in shapes {
        g.fill_t(&s, solid(col), t);
    }
}

impl Moon {
    pub fn build(back: &mut Canvas, front: &mut Canvas, atlas: &mut Atlas, l: super::Layout, painted: bool) -> Self {
        let (lw, lh, pt) = (l.w, l.h, l.portrait());
        let (mx, my) = (lw * 0.72656, lh * 0.29167);
        let wy = lh * 0.79861;
        let py = lh * 0.80556;
        let px = if pt { lw * 0.25 } else { 210.0 };
        let (tx, dy) = (if pt { lw * 0.78 } else { 474.0 }, py - 580.0);
        let mut r = Rng::new(33);
        let g = back;
        g.rect(0.0, 0.0, lw, lh, lin(0.0, 0.0, 0.0, lh, &[(0.0, c(0x060824)), (0.55, c(0x141846)), (1.0, c(0x2a2a66))]));
        for _ in 0..220 {
            let a = 0.2 + r.f() * 0.7;
            let s = 0.5 + r.f() * 1.4;
            let (x, y) = (r.f() * lw, r.f() * lh * 0.7);
            g.rect(x, y, s, s, solid(ca(255, 255, 255, a)));
        }
        g.glow(mx, my, 780.0, (255, 225, 160), 0.5);
        g.circle(mx, my, 112.0, rad(mx - 35.0, my - 35.0, mx, my, 112.0, &[(0.06, c(0xfffbea)), (0.6, c(0xf7e2a6)), (1.0, c(0xe6c172))]));
        for (x, y, rr) in [(40.0, -30.0, 16.0), (20.0, 40.0, 11.0), (65.0, 22.0, 8.0), (-50.0, -40.0, 9.0), (75.0, -10.0, 6.0)] {
            g.circle(mx + x, my + y, rr, solid(ca(176, 138, 72, 0.14)));
        }
        let tree = ca(150, 108, 52, 0.2);
        g.rect(mx - 36.0, my + 4.0, 6.0, 44.0, solid(tree));
        for (x, y, rr) in [(-33.0, -4.0, 18.0), (-50.0, 4.0, 12.0), (-16.0, 2.0, 13.0), (-40.0, -15.0, 12.0), (-24.0, -14.0, 11.0)] {
            g.circle(mx + x, my + y, rr, solid(tree));
        }

        super::landscape::paint(super::Kind::Moon, g, l);
        let h = front;
        h.hill(|x| lh * 0.75 + (x * 0.005 + 1.0).sin() * 20.0 + (x * 0.017).sin() * 8.0, solid(c(0x121541)));
        h.rect(0.0, wy, lw, lh - wy, lin(0.0, wy, 0.0, lh, &[(0.0, c(0x1a1e52)), (1.0, c(0x080a22))]));
        let ink = c(0x05061a);
        pavilion(h, px, py, ink, Transform::identity());
        pavilion(h, px, py, ca(5, 6, 26, 0.28), Transform::from_row(1.0, 0.0, 0.0, -1.0, 0.0, py * 2.0));
        let tdx = tx - 474.0;
        let mut pb = PathBuilder::new();
        pb.move_to(452.0 + tdx, 580.0 + dy);
        pb.quad_to(470.0 + tdx, 520.0 + dy, 462.0 + tdx, 470.0 + dy);
        pb.line_to(486.0 + tdx, 470.0 + dy);
        pb.quad_to(486.0 + tdx, 530.0 + dy, 510.0 + tdx, 580.0 + dy);
        if let Some(p) = pb.finish() {
            h.fill(&p, solid(ink));
        }
        for _ in 0..26 {
            let a = r.f() * PI;
            let d = r.f();
            let rr = 24.0 + r.f() * 30.0;
            h.circle(tx + a.cos() * d * 110.0, 450.0 + dy - a.sin() * d * 70.0, rr, solid(ink));
        }
        for _ in 0..14 {
            let x = tx - 74.0 + r.f() * 150.0;
            let (x1, y1) = (x + (r.f() - 0.5) * 6.0, 500.0 + dy + r.f() * 60.0);
            h.line(x, 470.0 + dy, x1, y1, solid(ink), 1.2);
        }

        let sky_uv = atlas.add(SL + 2.0, SL * 1.36 + 2.0, SL / 2.0 + 1.0, SL * 0.6 + 1.0, |cv| {
            let s = SL;
            let mut pb = PathBuilder::new();
            pb.move_to(-s * 0.42, -s * 0.6);
            pb.line_to(s * 0.42, -s * 0.6);
            pb.quad_to(s * 0.5, 0.0, s * 0.32, s * 0.6);
            pb.line_to(-s * 0.32, s * 0.6);
            pb.quad_to(-s * 0.5, 0.0, -s * 0.42, -s * 0.6);
            if let Some(p) = pb.finish() {
                cv.fill(&p, lin(0.0, -s * 0.6, 0.0, s * 0.6, &[(0.0, c(0xffd27e)), (1.0, c(0xe2622a))]));
            }
            cv.ellipse(0.0, s * 0.6, s * 0.3, s * 0.08, 0.0, solid(c(0xfff3c8)));
        });

        // ---- trẻ em rước đèn ông sao bên bờ hồ
        use super::figures::*;
        let star_uv = atlas.add(44.0, 50.0, 22.0, 20.0, |cv| star_lantern(cv, 13.0));
        let mut kids = Vec::new();
        let outfits = [
            Outfit { shirt: 0xff6b6b, pants: 0x495057, hair: Hair::Pigtails, hair_col: 0x2b2b2b, band: None, aodai: false, pattern: 0 },
            Outfit { shirt: 0x4dabf7, pants: 0x343a40, hair: Hair::Short, hair_col: 0x2b2b2b, band: None, aodai: false, pattern: 0 },
            Outfit { shirt: 0xffd43b, pants: 0xf1f3f5, hair: Hair::Short, hair_col: 0x2b2b2b, band: Some(0xe8590c), aodai: false, pattern: 0 },
            Outfit { shirt: 0xb197fc, pants: 0xf1f3f5, hair: Hair::Long, hair_col: 0x2b2b2b, band: None, aodai: true, pattern: 0xfff0f6 },
        ];
        let kx0 = if pt { lw * 0.36 } else { lw * 0.47 };
        // chỉ cảnh Tết âm lịch có người: không vẽ trẻ em rước đèn
        let n = 0;
        let _ = pt;
        for i in 0..n {
            let x = kx0 + i as f32 * 90.0;
            let g = wy - 2.0;
            // ánh đèn hắt lên người
            h.glow(x, g - 100.0, 230.0, (255, 170, 80), 0.25);
            let hand = chibi(h, x, g, 124.0 + (i % 2) as f32 * 10.0, Pose::RaiseHand, 1.0, outfits[i % 4]);
            // cán tre
            let tip = (hand.0 + 30.0, hand.1 - 42.0);
            h.line(hand.0, hand.1 + 6.0, tip.0, tip.1, solid(c(0x8a6a3a)), 2.4);
            kids.push((tip.0, tip.1, i as f32 * 1.3));
        }

        let mut sky = Vec::new();
        for _ in 0..18 {
            let (x, y, z, p) = (r.f() * lw, r.f() * lh, 0.35 + r.f() * 0.65, r.f() * TAU);
            sky.push(SkyLantern { x, y, z, p });
        }
        Moon {
            painted,
            w: lw,
            h: lh,
            mx,
            wy,
            clouds: vec![
                Cloud { x: lw * 0.46875, y: lh * 0.23611, w: 440.0, h: 70.0, v: 6.0 },
                Cloud { x: lw * 0.78125, y: lh * 0.35417, w: 360.0, h: 50.0, v: 9.0 },
                Cloud { x: lw * 0.15625, y: lh * 0.15278, w: 300.0, h: 44.0, v: 5.0 },
                Cloud { x: lw * 0.64063, y: lh * 0.43056, w: 520.0, h: 80.0, v: 4.0 },
            ],
            sky,
            sky_uv,
            kids,
            star_uv,
            rnd: Fast::seeded(),
        }
    }
}

impl Scene for Moon {
    fn frame(&mut self, t_live: f32, dt_live: f32, cfg: &Config, dl: &mut DrawList) {
        // Trung thu: chỉ đèn trời bay lên; mây, đèn ông sao, ánh trăng trên mặt hồ đứng yên
        let (t, dt) = (0.0f32, 0.0f32);
        dl.layer(Tex::Back);
        let cg = rgb8(150, 146, 215);
        for c in self.clouds.iter_mut().filter(|_| !self.painted) {
            c.x += c.v * dt;
            if c.x > self.w + c.w / 2.0 {
                c.x = -c.w / 2.0;
            }
            dl.glow(Blend::Alpha, c.x, c.y, c.w, c.h, cg, 0.42);
            dl.glow(Blend::Alpha, c.x, c.y - c.h * 0.3, c.w * 0.6, c.h, cg, 0.42);
        }
        if !self.painted {
            dl.layer(Tex::Front);
        }
        // đèn ông sao đung đưa trên cán, toả sáng ấm
        for (x, y, ph) in &self.kids {
            let swing = (t * 1.6 + ph).sin() * 0.18;
            let (lx, ly) = (x + swing.sin() * 14.0, y + 14.0);
            dl.glow(Blend::Add, lx, ly, 90.0, 90.0, rgb8(255, 170, 70), 0.35 + 0.08 * (t * 5.0 + ph).sin());
            dl.atlas(self.star_uv, lx, ly + 2.0, 44.0, 50.0, swing + (t * 0.8 + ph).sin() * 0.08, 1.0);
        }

        let gold = rgb8(255, 220, 150);
        for i in 0..if self.painted { 0 } else { 28 } {
            let fi = i as f32;
            let y = self.wy + 15.0 + fi * 4.4;
            let w = 26.0 + fi * 3.2 + (t * 2.0 + fi).sin() * 10.0;
            let x = self.mx + (t * 1.5 + fi * 0.8).sin() * 6.0;
            let a = (0.55 * (1.0 - fi / 30.0) * (0.55 + 0.45 * (t * 3.0 + fi * 1.7).sin())).max(0.0);
            dl.rect(Blend::Add, x, y + 0.8, w, 1.6, 0.0, gold, a);
        }

        if cfg.lanterns {
            let lg = rgb8(255, 160, 70);
            let (t, dt) = (t_live, dt_live);
            for l in &mut self.sky {
                l.y -= (6.0 + 14.0 * l.z) * dt;
                l.x += (t * 0.5 + l.p).sin() * 4.0 * dt;
                if l.y < -40.0 {
                    l.y = self.h + 20.0;
                    l.x = self.rnd.f() * self.w;
                }
                let visibility = if self.painted {
                    ((self.h * 0.60 - l.y) / (self.h * 0.09)).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                if visibility == 0.0 {
                    continue;
                }
                let s = 10.0 + 16.0 * l.z;
                let a = (0.55 + 0.45 * l.z) * visibility;
                dl.glow(Blend::Add, l.x, l.y, s * 5.0, s * 5.0, lg, 0.5 * a * (0.85 + 0.15 * (t * 4.0 + l.p).sin()));
                let k = s / SL;
                // tâm ảnh lệch xuống một chút vì có miệng đèn phía dưới
                dl.atlas(self.sky_uv, l.x, l.y + SL * 0.08 * k, (SL + 2.0) * k, (SL * 1.36 + 2.0) * k, 0.0, a);
            }
        }
    }
}
