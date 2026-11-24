//! Cảnh cho 4 mùa và các ngày lễ trong năm. Dùng chung một khung: bầu trời, mặt trời/trăng,
//! đồi xa – đồi gần, một hoạ tiết chính riêng cho từng dịp (cành đào, biển, rừng phong, cờ
//! Tổ quốc, bí ngô...), và hiệu ứng chuyển động riêng (cánh hoa, lá rơi, tuyết, pháo hoa,
//! trái tim, bóng bay, dơi...).

use std::f32::consts::{PI, TAU};
use tiny_skia::{Color, Path, PathBuilder, Transform};

use super::{Kind, Layout, Scene};
use crate::config::Config;
use crate::draw::*;
use crate::paint::*;

#[derive(Clone, Copy, PartialEq)]
enum Motif {
    Blossom,
    Sea,
    Maple,
    BareTrees,
    Skyline,
    Heart,
    Roses,
    Flag,
    FlagLotus,
    Rainbow,
    Pumpkins,
    Books,
}

#[derive(Clone, Copy, PartialEq)]
enum Fx {
    Breeze,
    Sparkles,
    Leaves,
    Snow,
    Fireworks,
    Fireflies,
}

struct Style {
    /// bầu trời: trên, giữa, dưới
    sky: [u32; 3],
    /// mặt trời/trăng: (x, y theo tỉ lệ khung, bán kính, màu lõi, màu viền, màu quầng, độ sáng quầng)
    sun: Option<(f32, f32, f32, u32, u32, (u8, u8, u8), f32)>,
    stars: usize,
    far: u32,
    near: u32,
    motif: Motif,
    fx: &'static [Fx],
    /// màu cho cánh hoa / lá / trái tim / bóng bay / pháo hoa
    cols: [u32; 4],
}

fn style(k: Kind) -> Style {
    match k {
        Kind::Spring => Style {
            sky: [0x9fd3ff, 0xd9ecff, 0xffe3ee],
            sun: Some((0.80, 0.20, 46.0, 0xfffbe8, 0xffe7a8, (255, 236, 190), 0.55)),
            stars: 0,
            far: 0xa9d9a0,
            near: 0x6fbf73,
            motif: Motif::Blossom,
            fx: &[],
            cols: [0xffb7cf, 0xff9dbd, 0xffd3e2, 0xfff0f5],
        },
        Kind::Summer => Style {
            sky: [0x2b86e3, 0x7cc4ff, 0xd7f1ff],
            sun: Some((0.78, 0.18, 58.0, 0xfffde6, 0xffd23f, (255, 220, 120), 0.75)),
            stars: 0,
            far: 0x5cb6a6,
            near: 0x2f8f78,
            motif: Motif::Sea,
            fx: &[],
            cols: [0xffffff, 0xfff3b0, 0xffe27a, 0xffffff],
        },
        Kind::Autumn => Style {
            sky: [0xf39a4e, 0xf9c58a, 0xffe7c4],
            sun: Some((0.62, 0.26, 52.0, 0xfff4d6, 0xffb35c, (255, 190, 110), 0.6)),
            stars: 0,
            far: 0xc98a4b,
            near: 0x8d5a2b,
            motif: Motif::Maple,
            fx: &[Fx::Leaves],
            cols: [0xe8571e, 0xf29a2e, 0xc9341b, 0xf5c242],
        },
        Kind::Winter => Style {
            sky: [0x8ea7bf, 0xc9d7e4, 0xeef3f8],
            sun: Some((0.72, 0.24, 40.0, 0xffffff, 0xe9eef5, (240, 245, 255), 0.4)),
            stars: 0,
            far: 0xdfe7ef,
            near: 0xf6f9fc,
            motif: Motif::BareTrees,
            fx: &[],
            cols: [0xffffff, 0xffffff, 0xffffff, 0xffffff],
        },
        Kind::NewYear => Style {
            sky: [0x050816, 0x10183a, 0x2a2350],
            sun: None,
            stars: 200,
            far: 0x141a3c,
            near: 0x0a0d22,
            motif: Motif::Skyline,
            fx: &[Fx::Fireworks],
            cols: [0xffd27a, 0xff6f91, 0x7fe0ff, 0xb6ff8a],
        },
        Kind::Valentine => Style {
            sky: [0xff8fb1, 0xffc2d4, 0xffe6ee],
            sun: None,
            stars: 0,
            far: 0xf7a1bd,
            near: 0xe9799e,
            motif: Motif::Heart,
            fx: &[],
            cols: [0xff4d79, 0xff7aa0, 0xe8285a, 0xffb3c8],
        },
        Kind::Women | Kind::VnWomen => Style {
            sky: [0xb9a3ff, 0xe3c9ff, 0xffe1ef],
            sun: Some((0.80, 0.22, 44.0, 0xfffaf0, 0xffd6e8, (255, 220, 236), 0.5)),
            stars: 0,
            far: 0xd6b4ef,
            near: 0xb48ad9,
            motif: Motif::Roses,
            fx: &[],
            cols: [0xff5c8a, 0xff8fb0, 0xd6336c, 0xffc2d6],
        },
        Kind::Reunify => Style {
            sky: [0xc9302c, 0xf2763d, 0xffd38a],
            sun: Some((0.42, 0.42, 60.0, 0xfff2c4, 0xffc244, (255, 200, 90), 0.7)),
            stars: 0,
            far: 0xb8462f,
            near: 0x7d2318,
            motif: Motif::Flag,
            fx: &[],
            cols: [0xffd23f, 0xff5a3c, 0xfff3c4, 0xffffff],
        },
        Kind::National => Style {
            sky: [0x1b2a6b, 0x7a3b8f, 0xf08a5d],
            sun: None,
            stars: 120,
            far: 0x3a2a5e,
            near: 0x1c173a,
            motif: Motif::FlagLotus,
            fx: &[],
            cols: [0xffd23f, 0xff4d3c, 0xfff3c4, 0xffa94d],
        },
        Kind::Children => Style {
            sky: [0x6ec6ff, 0xb5e3ff, 0xeaf8ff],
            sun: Some((0.84, 0.18, 46.0, 0xfffbe0, 0xffd54a, (255, 226, 120), 0.6)),
            stars: 0,
            far: 0x9ee08a,
            near: 0x5cc46a,
            motif: Motif::Rainbow,
            fx: &[],
            cols: [0xff5a5f, 0xffc93c, 0x4dabf7, 0x8ce99a],
        },
        Kind::Halloween => Style {
            sky: [0x170a2e, 0x3b1758, 0x7a3a5c],
            sun: Some((0.74, 0.26, 70.0, 0xffe8b0, 0xff9a3c, (255, 150, 60), 0.6)),
            stars: 90,
            far: 0x241238,
            near: 0x0e0718,
            motif: Motif::Pumpkins,
            fx: &[],
            cols: [0xff9a3c, 0xffd27a, 0xff7a1a, 0xffe0a0],
        },
        Kind::Teachers => Style {
            sky: [0xffcf8a, 0xffe6bf, 0xfff6e6],
            sun: Some((0.80, 0.22, 48.0, 0xfffbea, 0xffd27a, (255, 215, 140), 0.5)),
            stars: 0,
            far: 0xe8c79a,
            near: 0xc79a62,
            motif: Motif::Books,
            fx: &[],
            cols: [0xffd23f, 0xff9f43, 0xffb3c8, 0xfff0b0],
        },
        // Các cảnh có riêng (Giáng sinh, Tết, Trung thu) không dùng khung này
        _ => style(Kind::Spring),
    }
}

/// Trộn hai màu hex theo tỉ lệ t (0 = a, 1 = b).
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let m = |sh: u32| {
        let (x, y) = (((a >> sh) & 0xff) as f32, ((b >> sh) & 0xff) as f32);
        ((x + (y - x) * t).round() as u32) << sh
    };
    m(16) | m(8) | m(0)
}

fn rgb(hex: u32) -> (u8, u8, u8) {
    ((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn col(hex: u32, a: f32) -> Color {
    let (r, g, b) = rgb(hex);
    ca(r, g, b, a)
}

fn frgb(hex: u32) -> [f32; 3] {
    let (r, g, b) = rgb(hex);
    rgb8(r, g, b)
}

/// Hình trái tim tâm (cx, cy), cao khoảng 2s.
fn heart_path(cx: f32, cy: f32, s: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy + s);
    pb.cubic_to(cx - s * 1.6, cy - s * 0.1, cx - s * 0.9, cy - s * 1.25, cx, cy - s * 0.45);
    pb.cubic_to(cx + s * 0.9, cy - s * 1.25, cx + s * 1.6, cy - s * 0.1, cx, cy + s);
    pb.close();
    pb.finish()
}

/// Bông hoa 5 cánh.
fn flower(cv: &mut Canvas, x: f32, y: f32, r: f32, petal: Color, center: Color) {
    for i in 0..5 {
        let a = i as f32 * TAU / 5.0 - PI / 2.0;
        cv.circle(x + a.cos() * r * 0.62, y + a.sin() * r * 0.62, r * 0.55, solid(petal));
    }
    cv.circle(x, y, r * 0.32, solid(center));
}

/// Cành cây phân nhánh (đệ quy). `leaf` được gọi ở đầu các nhánh nhỏ.
fn branch(cv: &mut Canvas, x: f32, y: f32, ang: f32, len: f32, w: f32, depth: u32, r: &mut Rng, ink: Color, leaf: &mut dyn FnMut(&mut Canvas, f32, f32, &mut Rng)) {
    let (x1, y1) = (x + ang.cos() * len, y + ang.sin() * len);
    cv.line(x, y, x1, y1, solid(ink), w);
    if depth == 0 {
        leaf(cv, x1, y1, r);
        return;
    }
    let n = 2 + (r.f() * 1.6) as u32;
    for i in 0..n {
        let spread = 0.35 + r.f() * 0.4;
        let a = ang + (i as f32 - (n - 1) as f32 / 2.0) * spread + (r.f() - 0.5) * 0.3;
        branch(cv, x1, y1, a, len * (0.62 + r.f() * 0.18), (w * 0.68).max(0.8), depth - 1, r, ink, leaf);
    }
    if depth <= 2 {
        leaf(cv, x1, y1, r);
    }
}

/// Lá cờ đỏ sao vàng bay trong gió.
fn flag(cv: &mut Canvas, px: f32, py: f32, fw: f32, fh: f32) {
    cv.rect(px - 3.0, py, 6.0, fh * 3.6, solid(c(0xd9d2c5)));
    cv.circle(px, py - 4.0, 6.0, solid(c(0xffd23f)));
    let wave = |x: f32| (x / fw * TAU * 0.9).sin() * fh * 0.07;
    let mut pb = PathBuilder::new();
    pb.move_to(px + 3.0, py + 4.0 + wave(0.0));
    let mut x = 0.0;
    while x <= fw {
        pb.line_to(px + 3.0 + x, py + 4.0 + wave(x));
        x += 6.0;
    }
    let mut x = fw;
    while x >= 0.0 {
        pb.line_to(px + 3.0 + x, py + 4.0 + fh + wave(x));
        x -= 6.0;
    }
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, lin(px, py, px + fw, py + fh, &[(0.0, c(0xe8261d)), (0.55, c(0xd71b15)), (1.0, c(0xb5130e))]));
    }
    let (sx, sy) = (px + 3.0 + fw * 0.5, py + 4.0 + fh * 0.5 + wave(fw * 0.5));
    if let Some(p) = star_path(sx, sy, fh * 0.3, fh * 0.12, 5) {
        cv.fill(&p, solid(c(0xffde1a)));
    }
}

/// Bông sen nở.
fn lotus(cv: &mut Canvas, x: f32, y: f32, s: f32) {
    cv.ellipse(x, y + s * 0.15, s * 1.3, s * 0.28, 0.0, solid(c(0x2f7d4f)));
    for (a, l, cc) in [(-1.1f32, 0.9f32, 0xf7a6c4u32), (1.1, 0.9, 0xf7a6c4), (-0.55, 1.05, 0xf590b6), (0.55, 1.05, 0xf590b6), (0.0, 1.15, 0xfbc4d8)] {
        let mut pb = PathBuilder::new();
        let (tx, ty) = (x + a.sin() * s * l, y - a.cos() * s * l);
        pb.move_to(x, y);
        pb.quad_to(x + (a - 0.5).sin() * s * 0.7, y - (a - 0.5).cos() * s * 0.7, tx, ty);
        pb.quad_to(x + (a + 0.5).sin() * s * 0.7, y - (a + 0.5).cos() * s * 0.7, x, y);
        if let Some(p) = pb.finish() {
            cv.fill(&p, solid(c(cc)));
        }
    }
}

/// Chi tiết chuyển động riêng của từng cảnh (diều, thuyền, hải âu, khói, pháo sáng...).
enum Extra {
    /// diều bay theo gió, dây nối về tay người thả
    Kite { uv: [f32; 4], x: f32, y: f32, hand: (f32, f32), s: f32, ph: f32 },
    /// thuyền buồm trôi trên biển
    Boat { uv: [f32; 4], x: f32, y: f32, s: f32 },
    /// chim bay (hải âu, bồ câu): vỗ cánh
    Bird { x: f32, y: f32, ph: f32, s: f32, white: bool },
    /// khói ống khói
    Smoke { x: f32, y: f32 },
    /// que pháo sáng trên tay
    Sparkler { x: f32, y: f32 },
    /// quầng sáng nhấp nháy (đèn đường, bí ngô...)
    Light { x: f32, y: f32, s: f32, col: [f32; 3] },
    /// phù thuỷ cưỡi chổi bay ngang trăng
    Witch { uv: [f32; 4] },
}

/// Vẽ nhân vật minh hoạ (thay cho bóng người): cùng tham số với `person`, tự chọn trang phục
/// xoay vòng nhiều màu; cao hơn một chút vì đầu to.
/// Bật khi cảnh đang dựng có ảnh người riêng (art\<chủ đề>\nguoi.png).
static SKIP_PEOPLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn chibi_s(cv: &mut Canvas, x: f32, ground: f32, h: f32, pose: super::figures::Pose, dir: f32, _ink: Color) -> (f32, f32) {
    use super::figures::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let looks = [
        (0xf783ac, 0xc2255c, Hair::Pigtails, None),
        (0x4dabf7, 0x343a40, Hair::Short, None),
        (0xffd43b, 0xf1f3f5, Hair::Short, Some(0xfab005)),
        (0x9775fa, 0xf1f3f5, Hair::Long, Some(0xb197fc)),
        (0x69db7c, 0x2b8a3e, Hair::Bun, None),
        (0xff8787, 0x495057, Hair::Long, None),
    ];
    let i = N.fetch_add(1, Ordering::Relaxed);
    if SKIP_PEOPLE.load(Ordering::Relaxed) {
        // cảnh có ảnh người riêng: chỉ trả về vị trí tay (để nối dây diều...), không vẽ
        return (x + dir * h * 0.3, ground - h * 0.9);
    }
    let (sh, pa, hr, band) = looks[i % looks.len()];
    let aodai = pa == 0xf1f3f5 && matches!(hr, Hair::Long | Hair::Bun);
    chibi(cv, x, ground, h * 1.25, pose, dir, Outfit { shirt: sh, pants: pa, hair: hr, hair_col: 0x2b2b2b, band, aodai, pattern: 0xffe3ec })
}

struct P {
    x: f32,
    y: f32,
    rot: f32,
    vr: f32,
    s: f32,
    ph: f32,
    k: usize,
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

pub struct Festive {
    painted: bool,
    w: f32,
    h: f32,
    st: Style,
    parts: Vec<P>,
    sparks: Vec<Spark>,
    fw_t: f32,
    /// ảnh trong atlas: cánh hoa / lá / tim / bóng bay (theo 4 màu), dơi
    sprites: Vec<[f32; 4]>,
    /// chỗ để lấp lánh (mặt biển, cửa sổ...)
    glints: Vec<(f32, f32, f32)>,
    /// mây trôi (ban ngày): x, y, rộng, cao, tốc độ
    clouds: Vec<(f32, f32, f32, f32, f32)>,
    /// mặt trời (để vẽ tia nắng)
    sun: Option<(f32, f32)>,
    /// đốm sáng mờ trôi chậm (không khí lễ hội): x, y, cỡ, pha, màu
    bokeh: Vec<(f32, f32, f32, f32, usize)>,
    extras: Vec<Extra>,
    rnd: Fast,
}

const SP: f32 = 22.0;

impl Festive {
    pub fn build(kind: Kind, back: &mut Canvas, front: &mut Canvas, atlas: &mut Atlas, l: Layout, painted: bool) -> Self {
        let st = style(kind);
        let (lw, lh, pt) = (l.w, l.h, l.portrait());
        let mut r = Rng::new(kind as u32 * 97 + 11);

        // ---- bầu trời, sao, mặt trời / trăng
        let g = back;
        g.rect(0.0, 0.0, lw, lh, lin(0.0, 0.0, 0.0, lh, &[(0.0, c(st.sky[0])), (0.55, c(st.sky[1])), (1.0, c(st.sky[2]))]));
        for _ in 0..st.stars {
            let a = 0.25 + r.f() * 0.7;
            let s = 0.6 + r.f() * 1.4;
            let (x, y) = (r.f() * lw, r.f() * lh * 0.65);
            g.rect(x, y, s, s, solid(ca(255, 255, 255, a)));
        }
        if let Some((fx, fy, rr, core, edge, glow, ga)) = st.sun {
            let (sx, sy) = (lw * fx, lh * fy);
            g.glow(sx, sy, rr * 9.0, glow, ga);
            g.circle(sx, sy, rr, rad(sx - rr * 0.3, sy - rr * 0.3, sx, sy, rr, &[(0.0, c(core)), (1.0, c(edge))]));
        }
        if st.motif == Motif::Rainbow {
            let (cx, cy) = (lw * 0.5, lh * if pt { 0.62 } else { 0.86 });
            let base = if pt { lw * 0.62 } else { lw * 0.42 };
            for (i, cc) in [0xff6b6bu32, 0xffa94d, 0xffe066, 0x8ce99a, 0x74c0fc, 0x9775fa].iter().enumerate() {
                let rr = base - i as f32 * 16.0;
                let mut pb = PathBuilder::new();
                pb.push_circle(cx, cy, rr);
                if let Some(p) = pb.finish() {
                    g.stroke(&p, solid(col(*cc, 0.55)), 16.5);
                }
            }
        }
        let day = st.sun.is_some() && st.stars == 0;
        let haze = st.sky[2];
        // Màn dọc: phóng to cả phần mặt đất (núi, đồi, cảnh vật, đạo cụ) neo ở mép dưới giữa,
        // để đường chân trời nâng lên và cảnh lấp đầy khung cao, không còn trơ trọi dưới đáy.
        // Bầu trời, mặt trời, cờ, trái tim lớn, cành đào góc trên và khung tiền cảnh giữ nguyên.
        let zoom = if pt { 1.4 } else { 1.0 };
        let zt = Transform::from_translate(lw / 2.0, lh).pre_scale(zoom, zoom).pre_translate(-lw / 2.0, -lh);
        let zmap = |p: (f32, f32)| (lw / 2.0 + (p.0 - lw / 2.0) * zoom, lh + (p.1 - lh) * zoom);
        let gbase = g.base;
        g.base = gbase.pre_concat(zt);
        // dãy núi xa nhiều lớp: càng xa càng nhạt, ngả xanh (phối cảnh khí quyển)
        if day && !matches!(st.motif, Motif::Sea | Motif::Heart | Motif::Skyline) {
            for (i, (base, amp, tone)) in [(0.60f32, 120.0f32, 0.78f32), (0.63, 85.0, 0.6)].iter().enumerate() {
                let o = i as f32 * 2.3 + kind as u32 as f32 * 0.7;
                let colr = mix(mix(st.far, 0x5b7ea6, 0.35), st.sky[1], *tone);
                g.hill(
                    |x| lh * base - (x * 0.0055 + o).sin().abs().powf(0.7) * amp - (x * 0.019 + o * 1.7).sin().abs() * amp * 0.28,
                    lin(0.0, lh * (base - 0.17), 0.0, lh * base, &[(0.0, c(mix(colr, 0xffffff, 0.12))), (1.0, c(colr))]),
                );
            }
        }
        // làn sương mỏng ở đường chân trời (ánh sáng tán xạ như ảnh thật)
        g.rect(0.0, lh * 0.48, lw, lh * 0.3, lin(0.0, lh * 0.48, 0.0, lh * 0.78, &[(0.0, col(haze, 0.0)), (0.6, col(haze, if day { 0.55 } else { 0.25 })), (1.0, col(haze, 0.0))]));
        // đồi xa (nhạt dần vào sương) và đồi giữa: nhiều lớp cho có chiều sâu
        let far = st.far;
        g.hill(|x| lh * 0.64 + (x * 0.004 + 1.3).sin() * 26.0 + (x * 0.013).sin() * 10.0, lin(0.0, lh * 0.6, 0.0, lh * 0.85, &[(0.0, c(mix(far, haze, 0.45))), (1.0, c(far))]));
        let midc = mix(far, st.near, 0.5);
        if st.motif != Motif::Sea {
            g.hill(|x| lh * 0.72 + (x * 0.0052 + 3.1).sin() * 20.0 + (x * 0.011 + 0.7).sin() * 9.0, lin(0.0, lh * 0.68, 0.0, lh * 0.9, &[(0.0, c(mix(midc, haze, 0.25))), (1.0, c(midc))]));
        }
        // ---- cảnh sắc Việt Nam theo mùa
        match kind {
            // Vịnh Hạ Long: các đảo đá vôi nhô lên trên mặt biển, xa thì nhạt trong sương
            Kind::Summer => {
                let sea = lh * 0.70;
                let mut rk = Rng::new(52);
                for layer in 0..3 {
                    let fade = 0.55 - layer as f32 * 0.22;
                    let colr = mix(mix(0x2f6f5e, 0x1c4a3c, layer as f32 * 0.5), haze, fade);
                    let n = if pt { 4 } else { 7 };
                    for _ in 0..n {
                        let x = rk.f() * lw;
                        let w = (40.0 + rk.f() * 70.0) * (1.0 + layer as f32 * 0.35);
                        let hgt = (60.0 + rk.f() * 90.0) * (1.0 + layer as f32 * 0.4);
                        let base = sea + 2.0;
                        let mut pb = PathBuilder::new();
                        pb.move_to(x - w * 0.6, base);
                        pb.cubic_to(x - w * 0.62, base - hgt * 0.55, x - w * 0.42, base - hgt * 1.05, x - w * 0.08, base - hgt);
                        pb.cubic_to(x + w * 0.2, base - hgt * 1.08, x + w * 0.5, base - hgt * 0.7, x + w * 0.58, base);
                        pb.close();
                        if let Some(p) = pb.finish() {
                            g.fill(&p, lin(0.0, base - hgt, 0.0, base, &[(0.0, c(mix(colr, 0xffffff, 0.08))), (1.0, c(colr))]));
                        }
                        // cây bụi trên đỉnh đảo
                        for k in 0..5 {
                            g.circle(x - w * 0.25 + k as f32 * w * 0.12, base - hgt * (0.92 + (k % 2) as f32 * 0.05), w * 0.09, solid(c(mix(colr, 0x2b8a3e, 0.35))));
                        }
                    }
                }
            }
            // Ruộng bậc thang: các bờ ruộng uốn theo sườn đồi
            Kind::Spring => {
                for k in 0..16 {
                    let off = k as f32 * 7.5;
                    let mut pb = PathBuilder::new();
                    let mut x = 0.0;
                    let f = |x: f32| lh * 0.70 + off + (x * 0.0052 + 3.1 + k as f32 * 0.04).sin() * (20.0 - k as f32 * 0.6) + (x * 0.011 + 0.7).sin() * 9.0;
                    pb.move_to(0.0, f(0.0));
                    while x <= lw {
                        x += 10.0;
                        pb.line_to(x, f(x));
                    }
                    if let Some(p) = pb.finish() {
                        g.stroke(&p, solid(ca(240, 255, 200, 0.35)), 1.6);
                    }
                }
            }
            // Phố cổ Hà Nội: dãy nhà ống mái ngói, cửa chớp xanh
            Kind::Winter => {
                let mut rk = Rng::new(77);
                let mut x = -10.0;
                let walls = [0xe9c46a, 0xd8b98a, 0xc9a37a, 0xe3c9a0, 0xb8a58c];
                while x < lw {
                    let w = 46.0 + rk.f() * 30.0;
                    let hgt = 70.0 + rk.f() * 60.0;
                    let base = lh * 0.74 + (x * 0.0052 + 3.1).sin() * 8.0;
                    let wall = mix(walls[(rk.f() * 5.0) as usize % 5], haze, 0.35);
                    g.rect(x, base - hgt, w, hgt + 30.0, solid(c(wall)));
                    // mái ngói dốc
                    let mut pb = PathBuilder::new();
                    pb.move_to(x - 4.0, base - hgt);
                    pb.line_to(x + w * 0.5, base - hgt - 18.0);
                    pb.line_to(x + w + 4.0, base - hgt);
                    pb.close();
                    if let Some(p) = pb.finish() {
                        g.fill(&p, solid(c(mix(0x8d4a32, haze, 0.35))));
                    }
                    // cửa sổ chớp xanh
                    let mut wy = base - hgt + 14.0;
                    while wy < base - 14.0 {
                        g.rect(x + w * 0.25, wy, w * 0.5, 14.0, solid(c(mix(0x2f6b4f, haze, 0.35))));
                        g.rect(x + w * 0.5 - 0.5, wy, 1.0, 14.0, solid(c(mix(0x1c3f2f, haze, 0.35))));
                        wy += 26.0;
                    }
                    x += w + 1.0;
                }
            }
            _ => {}
        }

        super::landscape::paint(kind, g, l);
        g.base = gbase;
        // ---- tiền cảnh: đồi gần + hoạ tiết
        let h = front;
        let hbase = h.base;
        h.base = hbase.pre_concat(zt);
        let near = st.near;
        let ground = |x: f32| lh * 0.80 + (x * 0.0045 + 0.4).sin() * 18.0 + (x * 0.016 + 2.0).sin() * 7.0;
        let mut glints = Vec::new();
        match st.motif {
            Motif::Sea => {
                let sy = lh * 0.70;
                super::landscape::water(h, l, sy, [0x64b5bf, 0x184b66], [255, 229, 179], 0.56);
                h.hill(|x| lh * 0.90 + (x * 0.006).sin() * 10.0 - (x / lw) * 40.0, solid(c(0xf2d49b)));
                let (px, py) = (lw * if pt { 0.18 } else { 0.12 }, lh * 0.90);
                let mut pb = PathBuilder::new();
                pb.move_to(px - 8.0, py);
                pb.quad_to(px + 10.0, py - 140.0, px + 46.0, py - 250.0);
                pb.line_to(px + 56.0, py - 248.0);
                pb.quad_to(px + 24.0, py - 140.0, px + 10.0, py);
                if let Some(p) = pb.finish() {
                    h.fill(&p, solid(c(0x6b4a2b)));
                }
                let (tx, ty) = (px + 50.0, py - 250.0);
                for i in 0..7 {
                    let a = -PI * 0.95 + i as f32 * PI * 0.3;
                    let mut pb = PathBuilder::new();
                    pb.move_to(tx, ty);
                    pb.quad_to(tx + a.cos() * 70.0, ty + a.sin() * 70.0 - 30.0, tx + a.cos() * 130.0, ty + a.sin() * 60.0 + 20.0);
                    pb.quad_to(tx + a.cos() * 70.0, ty + a.sin() * 70.0 - 12.0, tx, ty + 6.0);
                    if let Some(p) = pb.finish() {
                        h.fill(&p, solid(c(0x2f7d3a)));
                    }
                }
                if let Some((fx, _, _, _, _, _, _)) = st.sun {
                    for _ in 0..70 {
                        let x = lw * fx + (r.f() - 0.5) * 220.0;
                        let y = sy + 6.0 + r.f() * (lh - sy) * 0.6;
                        glints.push((x, y, 0.6 + r.f()));
                    }
                }
            }
            _ => {
                // đồi gần: sáng ở đỉnh, tối dần xuống chân (như có ánh sáng chiếu từ trên)
                h.hill(ground, lin(0.0, lh * 0.76, 0.0, lh, &[(0.0, c(mix(near, 0xffffff, 0.12))), (1.0, c(mix(near, 0x000000, 0.22)))]));
            }
        }
        match st.motif {
            Motif::Blossom => {
                // cành đào từ hai góc trên
                let ink = c(0x5a3825);
                // góc trên bên phải, và một cành từ mép trái ở giữa (tránh đồng hồ ở góc trên trái)
                let n = 2;
                for side in 0..n {
                    let (x0, y0, a0) = if side == 1 { (-10.0, lh * if pt { 0.56 } else { 0.58 }, -0.12) } else { (lw + 10.0, -10.0, PI - 0.45) };
                    let mut leaf = |cv: &mut Canvas, x: f32, y: f32, r: &mut Rng| {
                        let pc = [0xffb7cf, 0xff9dbd, 0xffd3e2][(r.f() * 3.0) as usize % 3];
                        flower(cv, x + (r.f() - 0.5) * 8.0, y + (r.f() - 0.5) * 8.0, 7.0 + r.f() * 5.0, c(pc), c(0xd94f7a));
                    };
                    // cành từ góc trên bám theo khung màn hình, không phóng
                    h.base = if side == 1 { hbase.pre_concat(zt) } else { hbase };
                    branch(h, x0, y0, a0, if side == 1 { 100.0 } else { 150.0 }, 9.0, 4, &mut r, ink, &mut leaf);
                    h.base = hbase.pre_concat(zt);
                }
            }
            Motif::Maple => {
                // Hồ Gươm: mặt hồ phía trước, Tháp Rùa trên gò nổi, cầu Thê Húc đỏ
                let ly = lh * 0.86;
                super::landscape::water(h, l, ly, [0x839888, 0x345955], [255, 222, 158], 0.61);
                let (tx, tb) = (lw * if pt { 0.3 } else { 0.22 }, ly + 14.0);
                h.ellipse(tx, tb, 70.0, 12.0, 0.0, solid(c(0x4f6b3a)));
                for k in 0..5 {
                    h.circle(tx - 50.0 + k as f32 * 25.0, tb - 8.0, 14.0, solid(c(0x5c7c3a)));
                }
                let stone = c(0xd9c3a0);
                let dark = c(0x6b5a48);
                for (i, (w, hh)) in [(56.0f32, 34.0f32), (42.0, 24.0), (30.0, 20.0)].iter().enumerate() {
                    let y0 = tb - 8.0 - [0.0, 34.0, 58.0][i];
                    h.rect(tx - w / 2.0, y0 - hh, *w, *hh, solid(stone));
                    // cửa vòm
                    let n = 3 - i.min(1);
                    for k in 0..n {
                        let ax = tx - w / 2.0 + (k as f32 + 0.5) * w / n as f32;
                        h.round_rect(ax - 4.0, y0 - hh + 6.0, 8.0, hh - 8.0, 4.0, solid(dark));
                    }
                    h.rect(tx - w / 2.0 - 3.0, y0 - hh - 3.0, w + 6.0, 4.0, solid(c(0xbfa880)));
                }
                h.rect(tx - 6.0, tb - 8.0 - 58.0 - 20.0 - 14.0, 12.0, 14.0, solid(stone));
                // phản chiếu trên mặt hồ
                h.rect(tx - 28.0, tb + 6.0, 56.0, 40.0, lin(0.0, tb, 0.0, tb + 46.0, &[(0.0, ca(217, 195, 160, 0.35)), (1.0, ca(217, 195, 160, 0.0))]));
                // cầu Thê Húc: cầu cong sơn đỏ
                let (bx0, bx1, by) = (lw * if pt { 0.55 } else { 0.62 }, lw * if pt { 1.05 } else { 0.95 }, ly + 4.0);
                let mut pb = PathBuilder::new();
                pb.move_to(bx0, by);
                pb.quad_to((bx0 + bx1) / 2.0, by - 46.0, bx1, by);
                if let Some(p) = pb.finish() {
                    h.stroke(&p, solid(c(0xc92a2a)), 7.0);
                }
                let mut x = bx0 + 12.0;
                while x < bx1 - 6.0 {
                    let u = (x - bx0) / (bx1 - bx0);
                    let y = by - 46.0 * 4.0 * u * (1.0 - u) * 0.5;
                    h.line(x, y, x, y - 14.0, solid(c(0xe03131)), 2.0);
                    x += 14.0;
                }

                let trees = if pt { 4 } else { 7 };
                for i in 0..trees {
                    let x = lw * (0.06 + i as f32 / trees as f32 * 0.92) + (r.f() - 0.5) * 30.0;
                    let y = ground(x) + 6.0;
                    let th = 70.0 + r.f() * 60.0;
                    h.rect(x - 4.0, y - th, 8.0, th, solid(c(0x4a2c17)));
                    for _ in 0..16 {
                        let a = r.f() * TAU;
                        let d = r.f() * 1.0;
                        let cc = st.cols[(r.f() * 4.0) as usize % 4];
                        h.circle(x + a.cos() * d * 52.0, y - th - 26.0 + a.sin() * d * 38.0, 20.0 + r.f() * 16.0, solid(col(cc, 0.92)));
                    }
                }
            }
            Motif::BareTrees | Motif::Pumpkins => {
                let ink = if st.motif == Motif::Pumpkins { c(0x0b0614) } else { c(0x5d6670) };
                let trees = if pt { 2 } else { 3 };
                for i in 0..trees {
                    let x = lw * if pt { 0.26 + i as f32 * 0.48 } else { 0.1 + i as f32 * 0.38 } + (r.f() - 0.5) * 40.0;
                    let mut none = |_: &mut Canvas, _: f32, _: f32, _: &mut Rng| {};
                    branch(h, x, ground(x) + 6.0, -PI / 2.0 + (r.f() - 0.5) * 0.2, 90.0 + r.f() * 30.0, 9.0, 5, &mut r, ink, &mut none);
                }
                if st.motif == Motif::Pumpkins {
                    let n = if pt { 3 } else { 5 };
                    for i in 0..n {
                        let x = lw * if pt { 0.3 + i as f32 * 0.2 } else { 0.2 + i as f32 * 0.16 } + (r.f() - 0.5) * 20.0;
                        let s = 26.0 + r.f() * 16.0;
                        let y = ground(x) + 8.0 - s * 0.6;
                        h.glow(x, y, s * 5.0, (255, 140, 40), 0.35);
                        for (dx, k) in [(-0.45f32, 0.62f32), (0.45, 0.62), (0.0, 0.72)] {
                            h.ellipse(x + dx * s, y, s * k, s * 0.78, 0.0, solid(c(0xf07a16)));
                        }
                        h.rect(x - 3.0, y - s * 0.95, 6.0, s * 0.3, solid(c(0x3f6b25)));
                        let face = c(0xffd65a);
                        for dx in [-0.35f32, 0.35] {
                            let mut pb = PathBuilder::new();
                            pb.move_to(x + dx * s - s * 0.12, y - s * 0.08);
                            pb.line_to(x + dx * s + s * 0.12, y - s * 0.08);
                            pb.line_to(x + dx * s, y - s * 0.3);
                            pb.close();
                            if let Some(p) = pb.finish() {
                                h.fill(&p, solid(face));
                            }
                        }
                        h.ellipse(x, y + s * 0.25, s * 0.38, s * 0.12, 0.0, solid(face));
                    }
                }
            }
            Motif::Skyline => {
                let mut x = -10.0;
                while x < lw {
                    let bw = 40.0 + r.f() * 60.0;
                    let bh = 80.0 + r.f() * (if pt { 260.0 } else { 180.0 });
                    let y = lh * 0.86 - bh;
                    h.rect(x, y, bw, lh - y, solid(c(0x080a1c)));
                    let mut wy = y + 10.0;
                    while wy < lh * 0.84 {
                        let mut wx = x + 6.0;
                        while wx < x + bw - 8.0 {
                            if r.f() < 0.45 {
                                h.rect(wx, wy, 5.0, 7.0, solid(ca(255, 214, 140, 0.55 + r.f() * 0.4)));
                            }
                            wx += 11.0;
                        }
                        wy += 14.0;
                    }
                    x += bw + 2.0;
                }
                super::landscape::water(h, l, lh * 0.86, [0x25264d, 0x0c1229], [220, 178, 126], 0.52);
            }
            Motif::Heart => {
                h.base = hbase;
                let (cx, cy) = (lw * 0.5, lh * if pt { 0.42 } else { 0.45 });
                h.glow(cx, cy, 520.0, (255, 90, 130), 0.55);
                if let Some(p) = heart_path(cx, cy, 120.0) {
                    h.fill(&p, rad(cx - 40.0, cy - 60.0, cx, cy, 200.0, &[(0.0, c(0xff8fb0)), (1.0, c(0xe8285a))]));
                }
                for _ in 0..10 {
                    let (x, y, s) = (r.f() * lw, lh * 0.2 + r.f() * lh * 0.6, 10.0 + r.f() * 18.0);
                    if let Some(p) = heart_path(x, y, s) {
                        h.fill(&p, solid(col(st.cols[(r.f() * 4.0) as usize % 4], 0.5)));
                    }
                }
                h.base = hbase.pre_concat(zt);
            }
            Motif::Roses => {
                let (cx, by) = (lw * 0.5, lh * 0.93);
                for _ in 0..24 {
                    let a = -PI * 0.5 + (r.f() - 0.5) * 1.6;
                    let len = 140.0 + r.f() * 120.0;
                    let (x1, y1) = (cx + a.cos() * len * 0.8, by + a.sin() * len);
                    h.line(cx + (r.f() - 0.5) * 30.0, by, x1, y1, solid(c(0x3e7b3a)), 2.5);
                    h.ellipse(x1 + 10.0, y1 + 16.0, 12.0, 6.0, 0.6, solid(c(0x4f9a49)));
                    let rc = st.cols[(r.f() * 4.0) as usize % 4];
                    h.circle(x1, y1, 22.0, solid(c(rc)));
                    for k in 0..3 {
                        h.circle(x1 + (k as f32 - 1.0) * 5.0, y1 - 2.0, 9.0 - k as f32, solid(col(0x7a0f2e, 0.25)));
                    }
                }
                // giấy gói bó hoa
                let mut pb = PathBuilder::new();
                pb.move_to(cx - 90.0, by - 80.0);
                pb.line_to(cx + 90.0, by - 80.0);
                pb.line_to(cx + 18.0, lh + 10.0);
                pb.line_to(cx - 18.0, lh + 10.0);
                pb.close();
                if let Some(p) = pb.finish() {
                    h.fill(&p, lin(cx - 90.0, 0.0, cx + 90.0, 0.0, &[(0.0, c(0xfff0f6)), (0.5, c(0xffd6e7)), (1.0, c(0xf7b8d0))]));
                }
                h.round_rect(cx - 34.0, by - 30.0, 68.0, 16.0, 8.0, solid(c(0xd6336c)));
            }
            Motif::Flag | Motif::FlagLotus => {
                let (px, py) = (lw * if pt { 0.6 } else { 0.68 }, lh * if pt { 0.30 } else { 0.26 });
                let fw = if pt { 220.0 } else { 260.0 };
                h.base = hbase;
                flag(h, px, py, fw, fw * 0.66);
                h.base = hbase.pre_concat(zt);
                if st.motif == Motif::FlagLotus {
                    let wy = lh * 0.86;
                    h.rect(0.0, wy, lw, lh - wy, lin(0.0, wy, 0.0, lh, &[(0.0, c(0x2a3a7a)), (1.0, c(0x10173a))]));
                    let n = if pt { 4 } else { 6 };
                    for i in 0..n {
                        let x = lw * if pt { 0.2 + i as f32 * 0.6 / (n - 1) as f32 } else { 0.08 + i as f32 * 0.84 / (n - 1) as f32 };
                        lotus(h, x, wy + 18.0 + r.f() * 14.0, 26.0 + r.f() * 10.0);
                    }
                } else {
                    // chim bồ câu hoà bình (trên trời: không phóng)
                    h.base = hbase;
                    for i in 0..4 {
                        let (x, y) = (lw * (0.18 + i as f32 * 0.1), lh * (0.28 + (i % 2) as f32 * 0.06));
                        let mut pb = PathBuilder::new();
                        pb.move_to(x - 18.0, y);
                        pb.quad_to(x - 8.0, y - 12.0, x, y);
                        pb.quad_to(x + 8.0, y - 12.0, x + 18.0, y);
                        if let Some(p) = pb.finish() {
                            h.stroke(&p, solid(ca(255, 255, 255, 0.9)), 3.0);
                        }
                    }
                    h.base = hbase.pre_concat(zt);
                }
            }
            Motif::Books => {
                let (bx, by) = (lw * if pt { 0.12 } else { 0.08 }, lh * 0.86);
                let cols = [0xd9480f, 0x1c7ed6, 0x2f9e44, 0xf08c00, 0x862e9c];
                let mut y = by;
                for (i, cc) in cols.iter().enumerate() {
                    let w = 190.0 - i as f32 * 14.0;
                    let x = bx + (i as f32 * 7.0) % 20.0;
                    h.round_rect(x, y - 26.0, w, 26.0, 4.0, solid(c(*cc)));
                    h.rect(x + 8.0, y - 22.0, w - 16.0, 3.0, solid(ca(255, 255, 255, 0.35)));
                    y -= 27.0;
                }
                // bút
                h.round_rect(bx + 40.0, y - 10.0, 120.0, 8.0, 4.0, solid(c(0xffd43b)));
                // bình hoa tri ân
                let (vx, vy) = (lw * if pt { 0.72 } else { 0.80 }, lh * 0.88);
                for _ in 0..12 {
                    let a = -PI * 0.5 + (r.f() - 0.5) * 1.3;
                    let len = 90.0 + r.f() * 70.0;
                    let (x1, y1) = (vx + a.cos() * len * 0.7, vy - 60.0 + a.sin() * len);
                    h.line(vx, vy - 60.0, x1, y1, solid(c(0x3e7b3a)), 2.2);
                    let pc = st.cols[(r.f() * 4.0) as usize % 4];
                    flower(h, x1, y1, 14.0 + r.f() * 6.0, c(pc), c(0xc2410c));
                }
                h.round_rect(vx - 34.0, vy - 70.0, 68.0, 80.0, 18.0, lin(vx - 34.0, 0.0, vx + 34.0, 0.0, &[(0.0, c(0x74c0fc)), (1.0, c(0x1c7ed6))]));
            }
            Motif::Rainbow | Motif::Sea => {}
        }

        // ---- đường mòn uốn lượn chạy vào xa (tạo chiều sâu)
        if matches!(kind, Kind::Spring | Kind::Children | Kind::Teachers) {
            let pc = match kind {
                Kind::Autumn => 0xc8955a,
                Kind::Spring => 0xdcc9a0,
                _ => 0xe6d2a2,
            };
            let cx = lw * if pt { 0.5 } else { 0.56 };
            let top = (cx + 30.0, ground(cx + 30.0) + 2.0);
            let mut pb = PathBuilder::new();
            pb.move_to(cx - 150.0, lh + 20.0);
            pb.quad_to(cx - 300.0, lh * 0.9, top.0 - 5.0, top.1);
            pb.line_to(top.0 + 5.0, top.1);
            pb.quad_to(cx - 150.0, lh * 0.9, cx + 150.0, lh + 20.0);
            pb.close();
            if let Some(p) = pb.finish() {
                h.fill(&p, lin(0.0, top.1, 0.0, lh, &[(0.0, c(mix(pc, haze, 0.35))), (1.0, c(pc))]));
                h.stroke(&p, solid(ca(0, 0, 0, 0.08)), 2.0);
            }
            // sỏi nhỏ trên đường, gần to xa nhỏ
            let mut rk = Rng::new(9);
            for _ in 0..40 {
                let v = rk.f();
                let y = top.1 + (lh - top.1) * v * v;
                let spread = 10.0 + v * v * 150.0;
                let x = top.0 + (cx - top.0) * v * v - 40.0 * v * (1.0 - v) * 4.0 + (rk.f() - 0.5) * spread;
                h.ellipse(x, y, 1.0 + v * 5.0, 0.6 + v * 2.5, 0.0, solid(ca(120, 90, 60, 0.25)));
            }
        }

        // ---- con người và hoạt động riêng của từng dịp
        use super::figures::*;
        let sand = |x: f32| lh * 0.90 + (x * 0.006).sin() * 10.0 - (x / lw) * 40.0;
        let gnd = |x: f32| if st.motif == Motif::Sea { sand(x) } else { ground(x) } + 4.0;
        let ink = c(mix(st.near, 0x000000, 0.72));
        let mut extras: Vec<Extra> = Vec::new();
        // Ảnh người riêng (PNG nền trong suốt) thì dùng thay cho người vẽ bằng code
        let people_img = super::load_art(kind.id(), "nguoi.png");
        // Chỉ cảnh Tết âm lịch (tet.rs) có người vẽ bằng code; các dịp khác không vẽ người
        // (trừ khi người dùng tự đặt ảnh nguoi.png cho chủ đề đó).
        let people = people_img.is_some();
        SKIP_PEOPLE.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(img) = &people_img {
            let hh = if pt { 260.0 } else { 230.0 };
            let k = hh / img.height().max(1) as f32;
            let ww = img.width() as f32 * k;
            let x = lw * if pt { 0.5 } else { 0.32 };
            let t = h.base.pre_translate(x - ww / 2.0, gnd(x) + 6.0 - hh).pre_scale(k, k);
            h.pm.draw_pixmap(0, 0, img.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, t, None);
        }
        let at = |f: f32| lw * f;
        match kind {
            Kind::Spring | Kind::Children => {
                // trẻ em thả diều trên đồi
                let kites = [(0.62f32, 0.2f32, 0xff6b6bu32, 0xffd43b), (0.78, 0.3, 0x4dabf7, 0xffffff)];
                for (i, (fx, fy, a, b)) in kites.iter().enumerate() {
                    let cx = at(if pt { 0.35 + i as f32 * 0.3 } else { 0.48 + i as f32 * 0.14 });
                    if !people {
                        continue; // diều cần người cầm dây
                    }
                    let hand = chibi_s(h, cx, gnd(cx), 76.0, Pose::RaiseHand, 1.0, ink);
                    let uv = atlas.add(40.0, 90.0, 20.0, 34.0, |cv| kite(cv, 16.0, c(*a), c(*b)));
                    extras.push(Extra::Kite { uv, x: at(*fx), y: lh * fy, hand, s: 1.0, ph: i as f32 * 1.7 });
                }
                if kind == Kind::Children {
                    let x0 = at(if pt { 0.12 } else { 0.18 });
                    for (k, pose) in [Pose::Cheer, Pose::Stand, Pose::Cheer].iter().enumerate() {
                        let x = x0 + k as f32 * 70.0;
                        chibi_s(h, x, gnd(x), 76.0 + (k % 2) as f32 * 8.0, *pose, if k == 1 { -1.0 } else { 1.0 }, ink);
                    }
                }
            }
            Kind::Summer => {
                let uv = atlas.add(70.0, 80.0, 35.0, 76.0, |cv| sailboat(cv, 70.0));
                extras.push(Extra::Boat { uv, x: at(0.55), y: lh * 0.76, s: 1.0 });
                extras.push(Extra::Boat { uv, x: at(0.85), y: lh * 0.735, s: 0.6 });
                for i in 0..5 {
                    let (x, y, _speed) = (r.f() * lw, lh * (0.18 + r.f() * 0.25), r.f());
                    extras.push(Extra::Bird { x, y, ph: i as f32, s: 0.8 + r.f() * 0.5, white: true });
                }
                // ô che nắng + người nằm phơi nắng trên bãi cát
                let ux = at(if pt { 0.62 } else { 0.70 });
                let ug = sand(ux);
                h.line(ux, ug, ux - 8.0, ug - 110.0, solid(c(0x5a3a20)), 3.0);
                let mut pb = PathBuilder::new();
                pb.move_to(ux - 78.0, ug - 96.0);
                pb.quad_to(ux - 8.0, ug - 170.0, ux + 62.0, ug - 124.0);
                pb.close();
                if let Some(p) = pb.finish() {
                    h.fill(&p, lin(ux - 78.0, 0.0, ux + 62.0, 0.0, &[(0.0, c(0xff6b6b)), (0.5, c(0xfff5f5)), (1.0, c(0xff6b6b))]));
                }
                h.round_rect(ux - 60.0, ug - 8.0, 90.0, 8.0, 3.0, solid(c(0x4dabf7)));
                chibi_s(h, ux + 80.0, ug + 2.0, 92.0, Pose::Sit, -1.0, c(0x8a5a3a));
            }
            Kind::Autumn => {
                // xe đạp chở hoa – Hà Nội mùa thu
                let bx = at(if pt { 0.5 } else { 0.42 });
                let bg = gnd(bx);
                bicycle(h, bx, bg, 26.0, c(0x2b1a10));
                for k in 0..14 {
                    let (dx, dy) = ((k % 7) as f32 * 9.0 - 30.0, (k / 7) as f32 * -10.0);
                    let pc = [0xffd43b, 0xff8787, 0xffa94d, 0xfff3bf][k % 4];
                    h.circle(bx - 34.0 + dx, bg - 58.0 + dy, 7.0, solid(c(pc)));
                    h.circle(bx + 40.0 + dx * 0.5, bg - 52.0 + dy, 6.0, solid(c(pc)));
                }
                h.round_rect(bx - 66.0, bg - 54.0, 64.0, 12.0, 3.0, solid(c(0x8a5a2b)));
                let px = bx + 70.0;
                chibi_s(h, px, gnd(px), 112.0, Pose::Stand, -1.0, c(0x2b1a10));
                // nón lá cho cô bán hoa
                if people { let hh = 112.0 * 1.25; let cy = gnd(px) - hh * 0.9; let mut pb = PathBuilder::new(); pb.move_to(px - hh * 0.3, cy + hh * 0.06); pb.line_to(px, cy - hh * 0.2); pb.line_to(px + hh * 0.3, cy + hh * 0.06); pb.close(); if let Some(p) = pb.finish() { h.fill(&p, lin(px - hh * 0.3, 0.0, px + hh * 0.3, 0.0, &[(0.0, c(0xd9c08a)), (0.5, c(0xf3e2b3)), (1.0, c(0xc9ad74))])); } }
            }
            Kind::Winter => {
                let sx = at(if pt { 0.72 } else { 0.62 });
                snowman(h, sx, gnd(sx), 96.0);
                let hx = at(if pt { 0.3 } else { 0.8 });
                let top = cottage(h, hx, gnd(hx), 70.0, c(0x8a5a3a), c(0x5a2e1e), true);
                extras.push(Extra::Smoke { x: top.0, y: top.1 });
                chibi_s(h, hx - 120.0, gnd(hx - 120.0), 100.0, Pose::Walk, 1.0, c(0x2b3a4a));
            }
            Kind::NewYear => {
                // đám đông ngắm pháo hoa, vài người cầm pháo sáng
                let mut x = 20.0;
                let mut k = 0;
                while people && x < lw - 10.0 {
                    let gy = lh * 0.99;
                    let hh = 100.0 + r.f() * 34.0;
                    let pose = if k % 3 == 0 { Pose::Cheer } else if k % 5 == 1 { Pose::RaiseHand } else { Pose::Stand };
                    let hand = person(h, x, gy, hh, pose, if r.f() < 0.5 { 1.0 } else { -1.0 }, c(0x05060f));
                    if pose == Pose::RaiseHand {
                        extras.push(Extra::Sparkler { x: hand.0, y: hand.1 });
                    }
                    x += 40.0 + r.f() * 44.0;
                    k += 1;
                }
            }
            Kind::Valentine => {
                // đôi bạn trẻ ngồi ghế đá dưới cột đèn
                let bx = at(if pt { 0.5 } else { 0.28 });
                let bg = gnd(bx);
                h.rect(bx - 70.0, bg - 30.0, 140.0, 8.0, solid(c(0x6b2b3a)));
                h.rect(bx - 70.0, bg - 52.0, 140.0, 6.0, solid(c(0x6b2b3a)));
                for dx in [-60.0f32, 56.0] {
                    h.rect(bx + dx, bg - 30.0, 5.0, 30.0, solid(c(0x4a1a2a)));
                }
                let ink2 = c(0x4a0f22);
                chibi_s(h, bx - 22.0, bg - 26.0, 74.0, Pose::Sit, 1.0, ink2);
                chibi_s(h, bx + 22.0, bg - 26.0, 70.0, Pose::Sit, -1.0, ink2);
                // (tóc búi đã có sẵn trong nhân vật minh hoạ)
                let lx = bx + 120.0;
                h.rect(lx - 3.0, bg - 190.0, 6.0, 190.0, solid(c(0x3a1020)));
                h.round_rect(lx - 14.0, bg - 206.0, 28.0, 22.0, 6.0, solid(c(0x3a1020)));
                extras.push(Extra::Light { x: lx, y: bg - 192.0, s: 120.0, col: rgb8(255, 210, 150) });
                // bóng bay trái tim buộc vào ghế
                h.line(bx - 60.0, bg - 52.0, bx - 90.0, bg - 170.0, solid(ca(80, 20, 40, 0.8)), 1.2);
                if let Some(p) = heart_path(bx - 90.0, bg - 186.0, 18.0) {
                    h.fill(&p, solid(c(0xff2f6d)));
                }
            }
            Kind::Women | Kind::VnWomen if people => {
                // con gái tặng hoa mẹ
                let mx = at(if pt { 0.22 } else { 0.24 });
                let ink2 = c(0x3d1d5c);
                {
                    // mẹ: áo dài hồng, vấn khăn, hoạ tiết hoa
                    let _ = ink2;
                    chibi(h, mx, gnd(mx), 115.0, Pose::Stand, 1.0, Outfit { shirt: 0xf06595, pants: 0xf1f3f5, hair: Hair::Bun, hair_col: 0x2b2b2b, band: Some(0xc2255c), aodai: true, pattern: 0xfff0f6 });
                }
                // (tóc búi đã có sẵn trong nhân vật minh hoạ)
                let cx = mx + 70.0;
                let hand = chibi_s(h, cx, gnd(cx), 58.0, Pose::Give, -1.0, ink2);
                for k in 0..5 {
                    let a = -PI / 2.0 + (k as f32 - 2.0) * 0.35;
                    h.line(hand.0, hand.1, hand.0 + a.cos() * 22.0, hand.1 + a.sin() * 22.0, solid(c(0x3e7b3a)), 1.5);
                    h.circle(hand.0 + a.cos() * 24.0, hand.1 + a.sin() * 24.0, 6.0, solid(c(st.cols[k % 4])));
                }
            }
            Kind::Reunify | Kind::National => {
                // người dân vẫy cờ
                let n = if pt { 5 } else { 8 };
                for k in 0..if people { n } else { 0 } {
                    let x = at(0.04) + k as f32 * if pt { 50.0 } else { 56.0 };
                    let gy = if kind == Kind::National { lh * 0.86 - 2.0 } else { gnd(x) };
                    let hh = 60.0 + (k % 3) as f32 * 10.0;
                    let pose = if k % 2 == 0 { Pose::RaiseHand } else { Pose::Stand };
                    let hand = chibi_s(h, x, gy, hh, pose, 1.0, c(0x2a0806));
                    if pose == Pose::RaiseHand {
                        hand_flag(h, hand, 22.0, 1.0);
                    }
                }
                for i in 0..4 {
                    let (x, y, _speed) = (r.f() * lw, lh * (0.12 + r.f() * 0.2), r.f());
                    extras.push(Extra::Bird { x, y, ph: i as f32 * 1.3, s: 1.0, white: true });
                }
            }
            Kind::Halloween if people => {
                let uv = atlas.add(90.0, 50.0, 45.0, 25.0, |cv| {
                    let ink = c(0x0b0614);
                    cv.line(-40.0, 8.0, 34.0, 2.0, solid(ink), 3.0);
                    for k in 0..6 {
                        cv.line(-40.0, 8.0, -52.0, 2.0 + k as f32 * 3.0, solid(ink), 2.0);
                    }
                    let _ = person(cv, 0.0, 12.0, 46.0, Pose::Sit, 1.0, ink);
                    let mut pb = PathBuilder::new();
                    pb.move_to(-10.0, -22.0);
                    pb.line_to(-2.0, -44.0);
                    pb.line_to(8.0, -22.0);
                    pb.close();
                    if let Some(p) = pb.finish() {
                        cv.fill(&p, solid(ink));
                    }
                    cv.ellipse(-1.0, -22.0, 13.0, 3.0, 0.0, solid(ink));
                });
                extras.push(Extra::Witch { uv });
                // trẻ em hoá trang cầm đèn bí ngô
                let x0 = at(if pt { 0.55 } else { 0.66 });
                for k in 0..3 {
                    let x = x0 + k as f32 * 44.0;
                    let hand = chibi_s(h, x, gnd(x), 50.0, Pose::Give, -1.0, c(0x0b0614));
                    h.circle(hand.0 - 4.0, hand.1 + 8.0, 7.0, solid(c(0xf07a16)));
                    extras.push(Extra::Light { x: hand.0 - 4.0, y: hand.1 + 8.0, s: 40.0, col: rgb8(255, 150, 50) });
                }
            }
            Kind::Teachers => {
                // bảng đen trên giá, cô giáo và học trò tặng hoa
                let bx = at(if pt { 0.5 } else { 0.42 });
                let bg = gnd(bx);
                for dx in [-56.0f32, 56.0] {
                    h.line(bx + dx, bg, bx + dx * 0.8, bg - 150.0, solid(c(0x6b4a2b)), 5.0);
                }
                h.round_rect(bx - 80.0, bg - 160.0, 160.0, 100.0, 4.0, solid(c(0x8a5a2b)));
                h.rect(bx - 74.0, bg - 154.0, 148.0, 88.0, solid(c(0x2f5d3a)));
                for k in 0..3 {
                    let y = bg - 138.0 + k as f32 * 22.0;
                    h.line(bx - 60.0, y, bx - 60.0 + 70.0 + (k * 23 % 40) as f32, y + 2.0, solid(ca(255, 255, 255, 0.7)), 2.0);
                }
                let tx = bx + 110.0;
                let ink2 = c(0x4a2a10);
                if people {
                    // cô giáo: áo dài trắng, tóc dài
                    chibi(h, tx, gnd(tx), 128.0, Pose::Stand, -1.0, Outfit { shirt: 0xf8f9fa, pants: 0xf1f3f5, hair: Hair::Long, hair_col: 0x2b2b2b, band: None, aodai: true, pattern: 0x74c0fc });
                }
                // (tóc búi đã có sẵn trong nhân vật minh hoạ)
                let sx = tx + 90.0;
                let hand = chibi_s(h, sx, gnd(sx), 80.0, Pose::Give, -1.0, ink2);
                for k in 0..if people { 5 } else { 0 } {
                    let a = -PI / 2.0 + (k as f32 - 2.0) * 0.35;
                    h.line(hand.0, hand.1, hand.0 + a.cos() * 20.0, hand.1 + a.sin() * 20.0, solid(c(0x3e7b3a)), 1.5);
                    h.circle(hand.0 + a.cos() * 22.0, hand.1 + a.sin() * 22.0, 6.0, solid(c(st.cols[k % 4])));
                }
            }
            _ => {}
        }
        h.base = hbase;
        // vị trí gắn với mặt đất (thuyền, đèn, khói, pháo sáng, lấp lánh mặt biển) đi theo phép phóng
        if pt {
            for e in extras.iter_mut() {
                match e {
                    Extra::Kite { hand, .. } => *hand = zmap(*hand),
                    Extra::Boat { x, y, s, .. } => {
                        (*x, *y) = zmap((*x, *y));
                        *s *= zoom;
                    }
                    Extra::Smoke { x, y } | Extra::Sparkler { x, y } => (*x, *y) = zmap((*x, *y)),
                    Extra::Light { x, y, s, .. } => {
                        (*x, *y) = zmap((*x, *y));
                        *s *= zoom;
                    }
                    _ => {}
                }
            }
            for gl in glints.iter_mut() {
                (gl.0, gl.1) = zmap((gl.0, gl.1));
            }
        }

        // ---- tiền cảnh che khung ở hai góc dưới (gần ống kính: to, đậm) – tạo chiều sâu
        {
            let mut rk = Rng::new(31 + kind as u32);
            for side in [0.0f32, 1.0] {
                let bx = if side == 0.0 { -20.0 } else { lw + 20.0 };
                let sgn = if side == 0.0 { 1.0 } else { -1.0 };
                match kind {
                    Kind::Winter => {
                        for k in 0..7 {
                            let (x, y, rr) = (bx + sgn * (k as f32 * 34.0 + rk.f() * 20.0), lh - 10.0 - rk.f() * 40.0, 34.0 + rk.f() * 30.0);
                            h.circle(x, y + 6.0, rr, solid(c(0xc5d3e0)));
                            h.circle(x, y, rr, rad(x - rr * 0.3, y - rr * 0.4, x, y, rr * 1.1, &[(0.0, c(0xffffff)), (1.0, c(0xe3ebf3))]));
                        }
                        for _ in 0..6 {
                            let x0 = bx + sgn * rk.f() * 140.0;
                            h.line(x0, lh, x0 + sgn * (20.0 + rk.f() * 40.0), lh - 90.0 - rk.f() * 70.0, solid(c(0x4a3a30)), 2.2);
                        }
                    }
                    Kind::Autumn => {
                        for _ in 0..60 {
                            let (x, y) = (bx + sgn * rk.f() * 240.0, lh - rk.f() * 70.0);
                            let cc = st.cols[(rk.f() * 4.0) as usize % 4];
                            h.ellipse(x, y, 9.0 + rk.f() * 7.0, 5.0 + rk.f() * 3.0, rk.f() * 3.0, solid(c(mix(cc, 0x000000, 0.15))));
                        }
                        for _ in 0..16 {
                            let x0 = bx + sgn * rk.f() * 200.0;
                            h.line(x0, lh, x0 + sgn * (rk.f() - 0.3) * 30.0, lh - 60.0 - rk.f() * 80.0, solid(c(0x9a6a2a)), 2.0);
                        }
                    }
                    Kind::Summer => {
                        for _ in 0..22 {
                            let x0 = bx + sgn * rk.f() * 170.0;
                            let mut pb = PathBuilder::new();
                            pb.move_to(x0, lh + 4.0);
                            pb.quad_to(x0 + sgn * 10.0, lh - 60.0, x0 + sgn * (30.0 + rk.f() * 40.0), lh - 110.0 - rk.f() * 60.0);
                            if let Some(p) = pb.finish() {
                                h.stroke(&p, solid(c(0x6b8f3a)), 2.6);
                            }
                        }
                    }
                    _ => {
                        let gc = mix(st.near, 0x000000, if day { 0.35 } else { 0.6 });
                        for _ in 0..30 {
                            let x0 = bx + sgn * rk.f() * 200.0;
                            let hh = 50.0 + rk.f() * 110.0;
                            let mut pb = PathBuilder::new();
                            pb.move_to(x0, lh + 4.0);
                            pb.quad_to(x0 + sgn * 6.0, lh - hh * 0.6, x0 + sgn * (12.0 + rk.f() * 30.0), lh - hh);
                            if let Some(p) = pb.finish() {
                                h.stroke(&p, solid(c(gc)), 3.0);
                            }
                        }
                        if day {
                            for _ in 0..9 {
                                let (x, y) = (bx + sgn * (30.0 + rk.f() * 150.0), lh - 40.0 - rk.f() * 90.0);
                                let pc = [0xffd43b, 0xff8fab, 0xffffff, 0xb197fc][(rk.f() * 4.0) as usize % 4];
                                flower(h, x, y, 9.0 + rk.f() * 5.0, c(pc), c(0xf59f00));
                            }
                        }
                    }
                }
            }
        }

        // ---- ảnh nhỏ trong atlas cho hiệu ứng
        let fx_main = st.fx.first().copied();
        let breeze = (fx_main == Some(Fx::Breeze)).then(|| {
            atlas.add(72.0, 20.0, 36.0, 10.0, |cv| {
                for (dy, span) in [(-3.0f32, 29.0f32), (3.0, 21.0)] {
                    let mut p = PathBuilder::new();
                    p.move_to(-span, dy + 2.0);
                    p.cubic_to(-span * 0.45, dy - 6.0, span * 0.35, dy + 4.0, span, dy - 2.0);
                    if let Some(path) = p.finish() {
                        let tint = lin(-span, 0.0, span, 0.0, &[
                            (0.0, ca(250, 255, 242, 0.0)),
                            (0.5, ca(249, 255, 239, 0.84)),
                            (1.0, ca(235, 249, 228, 0.0)),
                        ]);
                        cv.stroke(&path, tint, 1.7);
                    }
                }
            })
        });
        let mut sprites = Vec::new();
        for cc in st.cols {
            let uv = match fx_main.unwrap_or(Fx::Sparkles) {
                Fx::Breeze => breeze.expect("spring wind stamp"),
                Fx::Leaves => super::particles::stamp(atlas, SP, cc, super::particles::Shape::Leaf),
                _ => atlas.add(SP, SP, SP / 2.0, SP / 2.0, |cv| {
                    cv.ellipse(0.0, 0.0, SP * 0.42, SP * 0.26, 0.3, rad(-SP * 0.1, -SP * 0.05, 0.0, 0.0, SP * 0.42, &[(0.0, col(0xffffff, 0.95)), (0.5, c(cc)), (1.0, c(cc))]));
                }),
            };
            sprites.push(uv);
        }
        // ---- hạt chuyển động
        let mut parts = Vec::new();
        let n = match fx_main.unwrap_or(Fx::Sparkles) {
            Fx::Breeze => 8,
            Fx::Snow => 320,
            Fx::Leaves => 46,
            _ => 0,
        };
        for _ in 0..n {
            parts.push(P {
                x: r.f() * lw,
                y: r.f() * lh,
                rot: r.f() * TAU,
                vr: (r.f() - 0.5) * 2.0,
                s: 0.5 + r.f() * 0.8,
                ph: r.f() * TAU,
                k: (r.f() * 4.0) as usize % 4,
            });
        }
        if glints.is_empty() && st.fx.contains(&Fx::Fireflies) {
            for _ in 0..40 {
                glints.push((r.f() * lw, lh * 0.45 + r.f() * lh * 0.45, 0.5 + r.f()));
            }
        }
        let day = st.sun.is_some() && st.stars == 0;
        let mut clouds = Vec::new();
        if day && st.motif != Motif::Heart {
            for i in 0..5 {
                clouds.push((r.f() * lw, lh * (0.08 + i as f32 * 0.07) + r.f() * 20.0, 260.0 + r.f() * 240.0, 40.0 + r.f() * 30.0, 4.0 + r.f() * 6.0));
            }
        }
        let sun = st.sun.map(|s| (lw * s.0, lh * s.1));
        let mut bokeh = Vec::new();
        if kind.is_holiday() {
            for _ in 0..22 {
                bokeh.push((r.f() * lw, r.f() * lh, 14.0 + r.f() * 34.0, r.f() * TAU, (r.f() * 4.0) as usize % 4));
            }
        }
        let sun = if painted { None } else { sun };
        if painted {
            clouds.clear();
            glints.clear();
            extras.clear();
            bokeh.truncate(10);
            if kind == Kind::Winter {
                // Hanoi's cold season is depicted with mist instead of snowfall.
                parts.clear();
            }
        }
        Festive { painted, w: lw, h: lh, st, parts, sparks: Vec::new(), fw_t: 0.5, sprites, glints, clouds, sun, bokeh, extras, rnd: Fast::seeded() }
    }
}

impl Scene for Festive {
    fn animated(&self) -> bool {
        !self.st.fx.is_empty()
    }

    fn frame(&mut self, t: f32, dt: f32, cfg: &Config, dl: &mut DrawList) {
        dl.layer(Tex::Back);
        let (w, h) = (self.w, self.h);
        // tia nắng toả ra từ mặt trời, xoay rất chậm
        if let (Some((sx, sy)), true) = (self.sun, self.st.stars == 0) {
            // tia mềm, thon ở gốc và tan dần ở ngọn, dài ngắn và sáng tối không đều như nắng thật;
            // gốc tia nằm trong quầng sáng nên không lộ mép quanh đĩa mặt trời
            let rr = self.st.sun.map(|s| s.2).unwrap_or(48.0);
            let tint = self.st.sun.map(|s| [s.5 .0 as f32 / 255.0, s.5 .1 as f32 / 255.0, s.5 .2 as f32 / 255.0]).unwrap_or([1.0; 3]);
            let warm = [(1.0 + tint[0]) * 0.5, (1.0 + tint[1]) * 0.5, (1.0 + tint[2]) * 0.5];
            const N: usize = 14;
            for i in 0..N {
                let k = i as f32;
                let j = ((k * 12.9898).sin() * 43758.545).fract().abs();
                let a = k * TAU / N as f32 + (j - 0.5) * 0.25;
                let len = (380.0 + j * 420.0) * (1.0 + 0.08 * (k * 1.7).sin());
                let w = 60.0 + j * 90.0;
                let al = (0.05 + 0.05 * j) * (0.75 + 0.25 * (k * 2.3).sin());
                dl.ray(sx, sy, len, w, a, warm, al);
            }
            // quầng sáng quanh đĩa mặt trời (đứng yên)
            let p = 1.0;
            dl.glow(Blend::Add, sx, sy, rr * 4.2 * p, rr * 4.2 * p, warm, 0.22);
            dl.glow(Blend::Add, sx, sy, rr * 2.4, rr * 2.4, [1.0; 3], 0.18);
        }
        // mây (đứng yên: chỉ giữ hiệu ứng chính của từng dịp)
        for c in &self.clouds {
            dl.glow(Blend::Alpha, c.0, c.1, c.2, c.3, [1.0; 3], 0.55);
            dl.glow(Blend::Alpha, c.0 - c.2 * 0.15, c.1 - c.3 * 0.3, c.2 * 0.55, c.3 * 1.1, [1.0; 3], 0.5);
        }
        if !self.painted {
            dl.layer(Tex::Front);
        }
        // đốm sáng mờ lễ hội: bỏ (không cần hiệu ứng phụ)
        self.bokeh.clear();
        for b in &mut self.bokeh {
            b.1 -= (4.0 + b.2 * 0.12) * dt;
            if b.1 < -40.0 {
                b.1 = h + 40.0;
            }
            let a = 0.10 + 0.08 * (t * 0.8 + b.3).sin();
            dl.glow(Blend::Add, b.0 + (t * 0.2 + b.3).sin() * 8.0, b.1, b.2, b.2, frgb(self.st.cols[b.4]), a.max(0.0));
        }
        // lấp lánh: mặt biển dưới nắng, đom đóm
        // ánh lấp lánh: đứng yên
        for (i, (x, y, s)) in self.glints.iter().enumerate() {
            let a = (0.5 + 0.5 * (i as f32 * 1.7).sin()).powi(3);
            if self.st.fx.contains(&Fx::Fireflies) {
                let (fx, fy) = (x + (t * 0.4 + i as f32).sin() * 18.0, y + (t * 0.3 + i as f32 * 0.7).cos() * 12.0);
                dl.glow(Blend::Add, fx, fy, 18.0 * s, 18.0 * s, rgb8(255, 210, 120), 0.6 * a);
            } else {
                dl.rect(Blend::Add, *x, *y, 10.0 * s, 1.6, 0.0, rgb8(255, 250, 220), 0.8 * a);
            }
        }
        let on = |f: Fx| match f {
            Fx::Snow => cfg.snow > 0.01,
            Fx::Fireworks => cfg.fireworks,
            Fx::Breeze | Fx::Leaves => cfg.petals,
            _ => true,
        };
        let fx0 = self.st.fx.first().copied().unwrap_or(Fx::Sparkles);
        if !self.st.fx.is_empty() && on(fx0) {
            let amount = if fx0 == Fx::Snow { cfg.snow.clamp(0.0, 2.0) * 0.5 } else { 1.0 };
            let shown = (self.parts.len() as f32 * amount) as usize;
            let wind = 6.0 + (t * 0.28).sin() * 9.0;
            for p in self.parts.iter_mut().take(shown) {
                match fx0 {
                    Fx::Breeze => {
                        p.x += (29.0 + p.s * 18.0 + wind * 0.4) * dt;
                        p.y += (t * 0.64 + p.ph).sin() * 8.0 * dt;
                    }
                    Fx::Snow => {
                        let depth = ((p.s - 0.5) * 1.25).powi(2);
                        p.y += (12.0 + 38.0 * depth) * dt;
                        p.x += (wind * (0.45 + depth) + (t * 0.6 + p.ph).sin() * 8.0) * dt;
                    }
                    Fx::Leaves => {
                        let flutter = (t * 1.2 + p.ph).sin();
                        p.y += (24.0 + 26.0 * p.s + flutter * 7.0) * dt;
                        p.x += (wind + flutter * 20.0) * dt;
                        p.rot += (p.vr + flutter * 0.52) * dt;
                    }
                    _ => {}
                }
                // ra khỏi khung thì quay lại từ phía đối diện
                if p.y > h + 40.0 {
                    p.y = -30.0;
                    p.x = self.rnd.f() * w;
                }
                if p.y < -60.0 {
                    p.y = h + 40.0;
                    p.x = self.rnd.f() * w;
                }
                if p.x > w + 60.0 {
                    p.x = -50.0;
                    p.y = if fx0 == Fx::Breeze {
                        h * (0.24 + self.rnd.f() * 0.48)
                    } else {
                        self.rnd.f() * h * 0.55
                    };
                }
                if p.x < -60.0 {
                    p.x = w + 50.0;
                }
                match fx0 {
                    Fx::Breeze => {
                        let width = 69.0 + p.s * 27.0;
                        let edge = (p.x / 90.0).min((w - p.x) / 105.0).clamp(0.0, 1.0);
                        let breath = 0.22 + 0.10 * (t * 0.80 + p.ph).sin();
                        dl.atlas(self.sprites[p.k], p.x, p.y, width, 19.0,
                            (t * 0.56 + p.ph).sin() * 0.11, edge * breath);
                    }
                    Fx::Snow => {
                        let depth = ((p.s - 0.5) * 1.25).powi(2);
                        let r = 0.95 + 3.0 * depth;
                        dl.disc(Blend::Alpha, p.x, p.y, r, r, 0.0, [0.92, 0.96, 1.0], 0.32 + 0.52 * depth);
                    }
                    Fx::Leaves => {
                        let flutter = (t * (1.1 + p.s * 0.42) + p.ph).sin();
                        let size = SP * (0.36 + 0.67 * p.s);
                        let edge = (p.y / 28.0).min((h - p.y) / 40.0).clamp(0.0, 1.0);
                        let opacity = (0.63 + 0.21 * flutter.abs()) * edge;
                        dl.atlas(self.sprites[p.k], p.x, p.y, size,
                            size * (0.29 + 0.71 * flutter.abs()), p.rot, opacity);
                    }
                    _ => {
                        let s = 0.7 + p.s;
                        let flip = (t * 1.6 + p.ph).sin();
                        dl.atlas(self.sprites[p.k], p.x, p.y, SP * s, SP * s * (0.35 + 0.65 * flip.abs()), p.rot, 0.95);
                    }
                }
            }
        }
        // chi tiết riêng của cảnh: diều, thuyền, chim, khói, pháo sáng, phù thuỷ
        for e in &mut self.extras {
            match e {
                Extra::Kite { uv, x, y, hand, s, ph } => {
                    let (kx, ky) = (*x + (t * 0.5 + *ph).sin() * 30.0, *y + (t * 0.8 + *ph).cos() * 16.0);
                    // dây diều: đoạn thẳng từ tay lên diều (võng nhẹ bằng 2 đoạn)
                    let (mx, my) = ((kx + hand.0) / 2.0 + 18.0, (ky + hand.1) / 2.0 + 30.0);
                    for (ax, ay, bx, by) in [(hand.0, hand.1, mx, my), (mx, my, kx, ky + 10.0)] {
                        let (dx, dy) = (bx - ax, by - ay);
                        dl.rect(Blend::Alpha, (ax + bx) / 2.0, (ay + by) / 2.0, (dx * dx + dy * dy).sqrt(), 1.0, dy.atan2(dx), rgb8(60, 60, 60), 0.7);
                    }
                    dl.atlas(*uv, kx, ky, 40.0 * *s, 90.0 * *s, (t * 1.1 + *ph).sin() * 0.25, 1.0);
                }
                Extra::Boat { uv, x, y, s } => {
                    dl.atlas(*uv, *x, *y - 40.0 * *s, 70.0 * *s, 80.0 * *s, 0.0, 1.0);
                }
                Extra::Bird { x, y, ph, s, white } => {
                    let flap = (*ph).sin() * 0.5;
                    let col = if *white { rgb8(255, 255, 255) } else { rgb8(30, 30, 40) };
                    let (bx, by) = (*x, *y);
                    dl.rect(Blend::Alpha, bx - 7.0 * *s, by - flap * 4.0, 15.0 * *s, 2.6, -0.45 - flap, col, 0.95);
                    dl.rect(Blend::Alpha, bx + 7.0 * *s, by - flap * 4.0, 15.0 * *s, 2.6, 0.45 + flap, col, 0.95);
                }
                Extra::Smoke { x, y } => {
                    for k in 0..6 {
                        let p = k as f32 / 6.0;
                        let sx = *x + (k as f32).sin() * 8.0 + p * 30.0;
                        let sy = *y - p * 120.0;
                        let size = 16.0 + p * 50.0;
                        dl.glow(Blend::Alpha, sx, sy, size, size, rgb8(235, 235, 240), 0.45 * (1.0 - p));
                    }
                }
                Extra::Sparkler { x, y } => {
                    for k in 0..6 {
                        let a = t * 9.0 + k as f32 * 1.1;
                        let rr = 6.0 + (t * 23.0 + k as f32 * 3.7).sin().abs() * 10.0;
                        dl.rect(Blend::Add, *x + a.cos() * rr * 0.5, *y + a.sin() * rr * 0.5, rr, 1.4, a, rgb8(255, 236, 170), 0.9);
                    }
                    dl.glow(Blend::Add, *x, *y, 36.0, 36.0, rgb8(255, 220, 140), 0.7);
                }
                Extra::Light { x, y, s, col } => {
                    let f = 1.0;
                    dl.glow(Blend::Add, *x, *y, *s * f, *s * f, *col, 0.55);
                }
                Extra::Witch { uv } => {
                    // bay ngang qua trăng theo vòng cung, lặp lại mỗi 24 giây
                    let p = (t / 24.0) % 1.0;
                    let wx = -100.0 + p * (w + 200.0);
                    let wy = h * 0.22 - (p * PI).sin() * h * 0.08 + (t * 2.0).sin() * 4.0;
                    dl.atlas(*uv, wx, wy, 90.0, 50.0, (t * 2.0).cos() * 0.06, 1.0);
                }
            }
        }

        // pháo hoa
        if self.st.fx.contains(&Fx::Fireworks) && cfg.fireworks {
            self.fw_t -= dt;
            if self.fw_t <= 0.0 {
                self.fw_t = 0.9 + self.rnd.f() * 1.3;
                let (x, y) = (w * (0.15 + self.rnd.f() * 0.7), h * (0.1 + self.rnd.f() * 0.3));
                let col = frgb(self.st.cols[(self.rnd.f() * 4.0) as usize % 4]);
                let n = 60;
                for i in 0..n {
                    let a = i as f32 / n as f32 * TAU + self.rnd.f() * 0.05;
                    let sp = 80.0 + self.rnd.f() * 70.0;
                    self.sparks.push(Spark { x, y, vx: a.cos() * sp, vy: a.sin() * sp, age: 0.0, life: 1.3 + self.rnd.f() * 0.7, c: col });
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
        // dơi bay qua (Halloween): thêm lớp hiệu ứng thứ hai
        if self.st.fx.len() > 1 && self.st.fx[1] == Fx::Fireflies {
            // đã vẽ cùng phần lấp lánh ở trên
        }
        let _ = Transform::identity();
    }
}
