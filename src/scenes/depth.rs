use super::{Kind, Layout};
use crate::paint::{c, ca, lin, pine, solid, Canvas};
use tiny_skia::PathBuilder;

fn blade(cv: &mut Canvas, x: f32, y: f32, tip: (f32, f32), width: f32, ink: u32) {
    let (tx, ty) = tip;
    let mut path = PathBuilder::new();
    path.move_to(x - width, y);
    path.quad_to(x + (tx - x) * 0.15, ty + (y - ty) * 0.45, tx, ty);
    path.quad_to(x + width, ty + (y - ty) * 0.40, x + width, y);
    path.close();
    if let Some(path) = path.finish() {
        cv.fill(&path, lin(x, y, tx, ty, &[(0.0, ca(20, 28, 38, 0.90)), (1.0, c(ink))]));
    }
}

fn flower_bed(cv: &mut Canvas, x: f32, y: f32, size: f32, leaf: u32, flowers: [u32; 2]) {
    cv.ellipse(x, y + 2.0, size * 0.64, size * 0.093, 0.0, solid(ca(23, 23, 34, 0.25)));
    for i in 0..11 {
        let dx = ((i * 23 % 11) as f32 / 5.0 - 1.0) * size * 0.46;
        let height = size * (0.22 + (i * 7 % 11) as f32 * 0.047);
        let stem = x + dx * 0.72;
        let (tx, ty) = (x + dx + size * 0.09, y - height);
        blade(cv, stem, y, (stem - size * 0.13, y - height * 0.70), 4.0, leaf);
        blade(cv, stem + 1.5, y, (tx + size * 0.11, y - height * 0.46), 3.0, leaf);
        cv.line(stem, y, tx, ty, solid(c(leaf)), 1.6);
        let r = size * (0.027 + (i % 4) as f32 * 0.008);
        for petal in 0..5 {
            let a = petal as f32 * std::f32::consts::TAU / 5.0;
            cv.ellipse(tx + a.cos() * r, ty + a.sin() * r, r, r * 0.70, a,
                solid(c(flowers[i % 2])));
        }
        cv.circle(tx, ty, r * 0.55, solid(c(0xe9b45c)));
    }
}

fn rocks(cv: &mut Canvas, x: f32, y: f32, size: f32, dark: u32, light: u32) {
    cv.ellipse(x, y + 2.0, size * 0.58, size * 0.075, 0.0, solid(ca(13, 19, 31, 0.28)));
    for (offset, raise, w, h) in [(-0.34, 0.01, 0.22, 0.13), (0.04, 0.07, 0.33, 0.24), (0.37, 0.0, 0.19, 0.10)] {
        let (cx, base) = (x + size * offset, y - size * raise);
        let mut path = PathBuilder::new();
        path.move_to(cx - w * size, base);
        path.line_to(cx - w * size * 0.75, base - h * size * 0.62);
        path.quad_to(cx - w * size * 0.22, base - h * size * 1.18,
            cx + w * size * 0.51, base - h * size * 0.86);
        path.line_to(cx + w * size, base - h * size * 0.23);
        path.close();
        if let Some(path) = path.finish() {
            cv.fill(&path, lin(cx, base - h * size, cx, base, &[(0.0, c(light)), (1.0, c(dark))]));
        }
    }
}

fn grasses(cv: &mut Canvas, x: f32, y: f32, size: f32, ink: u32) {
    for i in 0..12 {
        let offset = i as f32 / 11.0 - 0.5;
        let tip = x + size * (offset * 0.9 + 0.12);
        let ht = size * (0.35 + (i * 7 % 12) as f32 / 19.0);
        blade(cv, x + offset * size * 0.55, y, (tip, y - ht), 2.5 + (i % 4) as f32, ink);
    }
}

fn lantern(cv: &mut Canvas, x: f32, y: f32, size: f32, post: u32, light: u32) {
    let top = y - size;
    let lamp_x = x - size * 0.22;
    cv.ellipse(x, y, size * 0.20, size * 0.038, 0.0, solid(ca(20, 15, 28, 0.28)));
    cv.line(x, y, x, top, solid(c(post)), size * 0.054);
    cv.line(x, top, lamp_x, top, solid(c(post)), size * 0.045);
    cv.line(lamp_x, top, lamp_x, top + size * 0.12, solid(c(post)), 2.0);
    let lamp_y = top + size * 0.24;
    cv.glow(lamp_x, lamp_y, size * 1.2, (252, 163, 79), 0.34);
    cv.round_rect(lamp_x - size * 0.13, lamp_y - size * 0.12, size * 0.26, size * 0.24, size * 0.04,
        lin(lamp_x, lamp_y - size * 0.14, lamp_x, lamp_y + size * 0.14, &[(0.0, c(0xffd587)), (1.0, c(light))]));
    for dx in [-0.12, 0.0, 0.12] {
        cv.line(lamp_x + size * dx, lamp_y - size * 0.11, lamp_x + size * dx,
            lamp_y + size * 0.11, solid(c(post)), 1.5);
    }
}

pub(super) fn near(cv: &mut Canvas, kind: Kind, l: Layout) {
    let factor = (l.w / 900.0).clamp(0.80, 1.20);
    let (left, right) = (l.w * 0.12, l.w * 0.87);
    let y = l.h * 0.965;
    match kind {
        Kind::Spring | Kind::Children | Kind::Teachers | Kind::Women | Kind::VnWomen | Kind::Valentine => {
            let (leaves, blooms) = match kind {
                Kind::Spring => (0x526d45, [0xf59ea8, 0xffd7b0]),
                Kind::Children => (0x537944, [0xf5c46f, 0xe78587]),
                Kind::Teachers => (0x677747, [0xf5c660, 0xffe2ae]),
                Kind::Women => (0x67634e, [0xd4a8db, 0xf5cd83]),
                Kind::VnWomen => (0x537568, [0xea91b2, 0xf4c9b6]),
                _ => (0x614c5f, [0xd45c71, 0xf8b7a4]),
            };
            flower_bed(cv, left, y, 150.0 * factor, leaves, blooms);
            flower_bed(cv, right, y + 8.0, 176.0 * factor, leaves, blooms);
        }
        Kind::Summer | Kind::Autumn => {
            let (dark, light, ink) = if kind == Kind::Summer {
                (0x7a6455, 0xe3c89e, 0x7e9160)
            } else {
                (0x57594b, 0xa7a48a, 0xa7844b)
            };
            rocks(cv, left, y, 145.0 * factor, dark, light);
            grasses(cv, right, y + 8.0, 100.0 * factor, ink);
        }
        Kind::Winter | Kind::Noel => {
            rocks(cv, left, y, 148.0 * factor, 0x8199b8, 0xf2f5f8);
            let ridge = l.w * if kind == Kind::Noel { 0.96 } else { 0.88 };
            pine(cv, ridge + 28.0, y + 8.0, 142.0 * factor, solid(c(0x354e62)), Some(c(0xd2e4ef)));
            pine(cv, ridge - 30.0, y + 14.0, 90.0 * factor, solid(c(0x526c7e)), Some(c(0xe5edf5)));
        }
        Kind::Tet | Kind::Moon | Kind::NewYear | Kind::Halloween => {
            let (post, light) = match kind {
                Kind::Tet => (0x542b29, 0xdb4b32),
                Kind::Moon => (0x33263c, 0xd27a40),
                Kind::NewYear => (0x24293b, 0xf1a25a),
                _ => (0x291c30, 0xdc692c),
            };
            let lamp = if kind == Kind::Tet && l.portrait() { l.w * 0.07 } else { left };
            lantern(cv, lamp, y, 114.0 * factor, post, light);
            if kind != Kind::Tet {
                lantern(cv, l.w * 0.94, y + 5.0, 91.0 * factor, post, light);
            }
            rocks(cv, right, y + 7.0, 114.0 * factor, post, 0x716377);
        }
        Kind::National | Kind::Reunify => {
            let leaves = if kind == Kind::National { 0x476765 } else { 0x747149 };
            rocks(cv, left - 36.0, y, 120.0 * factor, 0x555260, 0xb0a591);
            flower_bed(cv, right, y + 6.0, 153.0 * factor, leaves, [0xd57c77, 0xf0c490]);
        }
    }
}

pub(super) fn distance(cv: &mut Canvas, kind: Kind, l: Layout) {
    let (top, low) = match kind {
        Kind::Spring | Kind::Children => ([157, 182, 147], [123, 157, 139]),
        Kind::Summer => ([122, 170, 175], [97, 152, 158]),
        Kind::Autumn | Kind::Teachers => ([216, 173, 124], [173, 143, 114]),
        Kind::Winter | Kind::Noel => ([136, 161, 188], [104, 132, 162]),
        Kind::Women | Kind::VnWomen | Kind::Valentine => ([203, 173, 204], [164, 132, 174]),
        Kind::National | Kind::Reunify => ([185, 141, 135], [143, 107, 125]),
        Kind::Tet => ([168, 86, 76], [111, 45, 55]),
        _ => ([82, 78, 123], [49, 51, 86]),
    };
    let [tr, tg, tb] = top;
    let [lr, lg, lb] = low;
    // Keep this layer near the horizon so the clock and desktop icons retain a quiet backdrop.
    let base = l.h * if l.portrait() { 0.66 } else { 0.65 };
    for i in 0..2 {
        let layer = i as f32;
        let y = base + l.h * layer * 0.055;
        cv.hill(
            |x| {
                let u = x / l.w;
                y - l.h * ((u * 9.0 + 0.8 + layer).sin().abs() * 0.056
                    + (u * 24.0 - layer).cos() * 0.012)
            },
            lin(0.0, y - l.h * 0.08, 0.0, y + l.h * 0.10, &[
                (0.0, ca(tr, tg, tb, 0.24 + layer * 0.11)),
                (1.0, ca(lr, lg, lb, 0.05)),
            ]),
        );
    }
}
