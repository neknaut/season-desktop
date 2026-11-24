//! Nhân vật và đồ vật dùng chung cho các cảnh: người (bóng đổ, nhiều tư thế), con lân,
//! xe đạp chở hoa, người tuyết, diều, thuyền buồm... Vẽ bằng hình vector trên canvas tĩnh;
//! các phần chuyển động (đầu lân, lửa, khói...) do từng cảnh vẽ mỗi khung hình.

use std::f32::consts::PI;
use tiny_skia::{Color, PathBuilder};

use crate::paint::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Pose {
    /// đứng thẳng, hai tay buông
    Stand,
    /// đang bước đi
    Walk,
    /// ngồi bệt dưới đất, hai tay đưa ra trước (gói bánh, sưởi lửa)
    Sit,
    /// ngồi xổm, một tay đưa vào (trông lửa)
    Squat,
    /// giơ một tay lên cao (vẫy cờ, cầm que diều, cầm đèn)
    RaiseHand,
    /// đưa một tay ra trước (tặng hoa)
    Give,
    /// hai tay giơ lên (reo hò xem pháo hoa)
    Cheer,
}

/// Một nét tay/chân bo tròn đi qua các điểm.
fn limb(cv: &mut Canvas, pts: &[(f32, f32)], w: f32, ink: Color) {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    if let Some(p) = pb.finish() {
        cv.stroke(&p, solid(ink), w);
    }
}

/// Vẽ một người cao `h`, chân chạm đất tại (x, ground), quay mặt theo `dir` (1 = phải, -1 = trái).
/// Trả về vị trí bàn tay "chính" (để gắn đồ cầm: cờ, đèn, hoa, diều...).
pub fn person(cv: &mut Canvas, x: f32, ground: f32, h: f32, pose: Pose, dir: f32, ink: Color) -> (f32, f32) {
    let head_r = h * 0.085;
    let (sit, squat) = (pose == Pose::Sit, pose == Pose::Squat);
    // độ cao các khớp (ngồi thì thấp xuống)
    let drop = if sit { h * 0.40 } else if squat { h * 0.30 } else { 0.0 };
    let top = ground - h + drop;
    let head = (x + if squat { dir * h * 0.05 } else { 0.0 }, top + head_r);
    let neck_y = head.1 + head_r * 1.05;
    let sh_y = neck_y + h * 0.03;
    let hip_y = ground - h * 0.47 + drop;
    let (sw, hw) = (h * 0.11, h * 0.085);
    let lean = if squat { dir * h * 0.06 } else { 0.0 };
    // thân (áo): hình thang bo
    let mut pb = PathBuilder::new();
    pb.move_to(x - sw + lean, sh_y);
    pb.line_to(x + sw + lean, sh_y);
    pb.quad_to(x + sw * 1.05, (sh_y + hip_y) / 2.0, x + hw, hip_y);
    pb.line_to(x - hw, hip_y);
    pb.quad_to(x - sw * 1.05, (sh_y + hip_y) / 2.0, x - sw + lean, sh_y);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(ink));
    }
    cv.rect(x - h * 0.03 + lean * 0.5, neck_y - head_r * 0.2, h * 0.06, sh_y - neck_y + head_r * 0.3, solid(ink));
    cv.circle(head.0, head.1, head_r, solid(ink));
    let (lw_, aw) = (h * 0.075, h * 0.058);
    // chân
    let leg = h * 0.47;
    match pose {
        Pose::Sit => {
            for s in [-1.0f32, 1.0] {
                limb(cv, &[(x + s * hw * 0.5, hip_y), (x + dir * leg * 0.55 + s * 3.0, ground - h * 0.12), (x + dir * leg * 0.95 + s * 4.0, ground - 2.0)], lw_, ink);
            }
        }
        Pose::Squat => {
            for s in [-1.0f32, 1.0] {
                limb(cv, &[(x + s * hw * 0.5, hip_y), (x + dir * leg * 0.45 + s * 4.0, hip_y - h * 0.08), (x + dir * leg * 0.25 + s * 5.0, ground - 2.0)], lw_, ink);
            }
        }
        Pose::Walk => {
            limb(cv, &[(x - hw * 0.3, hip_y), (x - dir * leg * 0.25, ground - leg * 0.5), (x - dir * leg * 0.4, ground - 2.0)], lw_, ink);
            limb(cv, &[(x + hw * 0.3, hip_y), (x + dir * leg * 0.2, ground - leg * 0.5), (x + dir * leg * 0.35, ground - 2.0)], lw_, ink);
        }
        _ => {
            for s in [-1.0f32, 1.0] {
                limb(cv, &[(x + s * hw * 0.55, hip_y), (x + s * hw * 0.6, ground - 2.0)], lw_, ink);
            }
        }
    }
    // tay
    let arm = h * 0.36;
    let (fx, bx) = (x + dir * sw * 0.8 + lean, x - dir * sw * 0.8 + lean);
    let hand = match pose {
        Pose::Stand | Pose::Walk => {
            limb(cv, &[(bx, sh_y + 3.0), (bx - dir * 4.0, sh_y + arm)], aw, ink);
            let hnd = (fx + dir * 4.0, sh_y + arm);
            limb(cv, &[(fx, sh_y + 3.0), hnd], aw, ink);
            hnd
        }
        Pose::Sit | Pose::Give => {
            limb(cv, &[(bx, sh_y + 3.0), (bx + dir * arm * 0.4, sh_y + arm * 0.75), (x + dir * arm * 0.5, sh_y + arm * 0.85)], aw, ink);
            let hnd = (fx + dir * arm * 0.95, sh_y + arm * 0.45);
            limb(cv, &[(fx, sh_y + 3.0), (fx + dir * arm * 0.5, sh_y + arm * 0.55), hnd], aw, ink);
            hnd
        }
        Pose::Squat => {
            limb(cv, &[(bx, sh_y + 3.0), (bx + dir * arm * 0.3, sh_y + arm * 0.8)], aw, ink);
            let hnd = (fx + dir * arm * 0.9, sh_y + arm * 0.7);
            limb(cv, &[(fx, sh_y + 3.0), (fx + dir * arm * 0.45, sh_y + arm * 0.6), hnd], aw, ink);
            hnd
        }
        Pose::RaiseHand => {
            limb(cv, &[(bx, sh_y + 3.0), (bx - dir * 4.0, sh_y + arm)], aw, ink);
            let hnd = (fx + dir * arm * 0.35, sh_y - arm * 0.85);
            limb(cv, &[(fx, sh_y + 3.0), (fx + dir * arm * 0.3, sh_y - arm * 0.3), hnd], aw, ink);
            hnd
        }
        Pose::Cheer => {
            limb(cv, &[(bx, sh_y + 3.0), (bx - dir * arm * 0.35, sh_y - arm * 0.3), (bx - dir * arm * 0.5, sh_y - arm * 0.85)], aw, ink);
            let hnd = (fx + dir * arm * 0.5, sh_y - arm * 0.85);
            limb(cv, &[(fx, sh_y + 3.0), (fx + dir * arm * 0.35, sh_y - arm * 0.3), hnd], aw, ink);
            hnd
        }
    };
    hand
}

/// Lá cờ đỏ sao vàng nhỏ cầm tay, cán dài từ bàn tay lên.
pub fn hand_flag(cv: &mut Canvas, hand: (f32, f32), s: f32, wave: f32) {
    let (x, y) = hand;
    cv.line(x, y + s * 0.2, x, y - s * 1.6, solid(c(0x6b4a2b)), s * 0.07);
    let mut pb = PathBuilder::new();
    let top = y - s * 1.6;
    pb.move_to(x, top);
    pb.quad_to(x + s * 0.5, top - s * 0.12 * wave, x + s * 1.1, top + s * 0.05 * wave);
    pb.line_to(x + s * 1.1, top + s * 0.75 + s * 0.05 * wave);
    pb.quad_to(x + s * 0.5, top + s * 0.62 - s * 0.12 * wave, x, top + s * 0.7);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(c(0xda251d)));
    }
    if let Some(p) = star_path(x + s * 0.55, top + s * 0.36, s * 0.2, s * 0.08, 5) {
        cv.fill(&p, solid(c(0xffde1a)));
    }
}

/// Bánh chưng xanh, buộc lạt.
pub fn banh_chung(cv: &mut Canvas, x: f32, y: f32, s: f32) {
    cv.rect(x - s / 2.0, y - s, s, s, lin(x - s / 2.0, y - s, x + s / 2.0, y, &[(0.0, c(0x4f9a3a)), (1.0, c(0x2d6a22))]));
    for k in [0.33f32, 0.66] {
        cv.rect(x - s / 2.0 + s * k - 1.0, y - s, 2.0, s, solid(c(0xe9d9a6)));
        cv.rect(x - s / 2.0, y - s + s * k - 1.0, s, 2.0, solid(c(0xe9d9a6)));
    }
}

/// Xe đạp (khung + bánh) đứng ở (x, ground), dài khoảng 2.2 * r.
pub fn bicycle(cv: &mut Canvas, x: f32, ground: f32, r: f32, ink: Color) {
    let (bx, fx, wy) = (x - r * 1.1, x + r * 1.1, ground - r);
    for wx in [bx, fx] {
        let mut pb = PathBuilder::new();
        pb.push_circle(wx, wy, r);
        if let Some(p) = pb.finish() {
            cv.stroke(&p, solid(ink), r * 0.1);
        }
        cv.circle(wx, wy, r * 0.12, solid(ink));
    }
    let (px, py) = (x - r * 0.1, wy);
    let (sx, sy) = (x - r * 0.35, wy - r * 1.05);
    let (hx, hy) = (fx - r * 0.25, wy - r * 1.2);
    limb(cv, &[(bx, wy), (px, py), (sx, sy), (bx, wy)], r * 0.09, ink);
    limb(cv, &[(px, py), (hx, hy - r * 0.05), (sx + r * 0.1, sy + r * 0.05)], r * 0.09, ink);
    limb(cv, &[(hx, hy), (fx, wy)], r * 0.09, ink);
    limb(cv, &[(hx - r * 0.25, hy - r * 0.15), (hx + r * 0.15, hy - r * 0.2)], r * 0.08, ink);
    cv.ellipse(sx - r * 0.05, sy - r * 0.08, r * 0.22, r * 0.07, 0.0, solid(ink));
}

/// Người tuyết.
pub fn snowman(cv: &mut Canvas, x: f32, ground: f32, s: f32) {
    let body = |cv: &mut Canvas, cy: f32, r: f32| {
        cv.circle(x, cy, r, rad(x - r * 0.35, cy - r * 0.35, x, cy, r * 1.1, &[(0.0, c(0xffffff)), (1.0, c(0xd5e1ee))]));
    };
    body(cv, ground - s * 0.42, s * 0.45);
    body(cv, ground - s * 1.05, s * 0.32);
    body(cv, ground - s * 1.5, s * 0.22);
    let hy = ground - s * 1.5;
    cv.circle(x - s * 0.07, hy - s * 0.04, s * 0.025, solid(c(0x1a1a1a)));
    cv.circle(x + s * 0.07, hy - s * 0.04, s * 0.025, solid(c(0x1a1a1a)));
    let mut pb = PathBuilder::new();
    pb.move_to(x, hy + s * 0.01);
    pb.line_to(x + s * 0.2, hy + s * 0.04);
    pb.line_to(x, hy + s * 0.06);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(c(0xf08c00)));
    }
    cv.round_rect(x - s * 0.26, hy + s * 0.13, s * 0.52, s * 0.09, s * 0.04, solid(c(0xd6336c)));
    cv.rect(x + s * 0.1, hy + s * 0.18, s * 0.09, s * 0.25, solid(c(0xd6336c)));
    cv.rect(x - s * 0.2, hy - s * 0.32, s * 0.4, s * 0.06, solid(c(0x222222)));
    cv.rect(x - s * 0.13, hy - s * 0.55, s * 0.26, s * 0.25, solid(c(0x222222)));
    for k in 0..3 {
        cv.circle(x, ground - s * 1.15 + k as f32 * s * 0.12, s * 0.025, solid(c(0x222222)));
    }
    for d in [-1.0f32, 1.0] {
        limb(cv, &[(x + d * s * 0.28, ground - s * 1.05), (x + d * s * 0.6, ground - s * 1.3), (x + d * s * 0.7, ground - s * 1.42)], s * 0.035, c(0x5a3a20));
    }
}

/// Ngôi nhà nhỏ có cửa sổ sáng đèn và ống khói (khói do cảnh vẽ thêm). Trả về đỉnh ống khói.
pub fn cottage(cv: &mut Canvas, x: f32, ground: f32, s: f32, wall: Color, roof: Color, snow_roof: bool) -> (f32, f32) {
    let (w, h) = (s * 1.6, s * 0.9);
    cv.rect(x - w / 2.0, ground - h, w, h, solid(wall));
    let mut pb = PathBuilder::new();
    pb.move_to(x - w / 2.0 - s * 0.15, ground - h);
    pb.line_to(x, ground - h - s * 0.75);
    pb.line_to(x + w / 2.0 + s * 0.15, ground - h);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(roof));
    }
    if snow_roof {
        let mut pb = PathBuilder::new();
        pb.move_to(x - w / 2.0 - s * 0.15, ground - h);
        pb.line_to(x, ground - h - s * 0.75);
        pb.line_to(x + w / 2.0 + s * 0.15, ground - h);
        if let Some(p) = pb.finish() {
            cv.stroke(&p, solid(c(0xffffff)), s * 0.1);
        }
    }
    let chim = (x + w * 0.25, ground - h - s * 0.55);
    cv.rect(chim.0 - s * 0.08, chim.1, s * 0.16, s * 0.4, solid(roof));
    // cửa sổ sáng + cửa ra vào
    cv.glow(x - w * 0.22, ground - h * 0.55, s * 1.4, (255, 190, 100), 0.45);
    cv.rect(x - w * 0.32, ground - h * 0.72, s * 0.32, s * 0.3, solid(c(0xffd27a)));
    cv.rect(x - w * 0.32 + s * 0.15, ground - h * 0.72, s * 0.02, s * 0.3, solid(roof));
    cv.rect(x + w * 0.08, ground - h * 0.62, s * 0.3, h * 0.62, solid(c(0x5a3a22)));
    (chim.0, chim.1)
}

/// Thuyền buồm (vẽ tại gốc (0,0) = đáy thuyền giữa, cao khoảng s).
pub fn sailboat(cv: &mut Canvas, s: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(-s * 0.55, -s * 0.12);
    pb.line_to(s * 0.55, -s * 0.12);
    pb.line_to(s * 0.4, 0.0);
    pb.line_to(-s * 0.4, 0.0);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(c(0x8b4a2b)));
    }
    cv.rect(-s * 0.02, -s * 1.0, s * 0.04, s * 0.9, solid(c(0x5a3a20)));
    let mut pb = PathBuilder::new();
    pb.move_to(s * 0.03, -s * 0.95);
    pb.quad_to(s * 0.4, -s * 0.5, s * 0.45, -s * 0.18);
    pb.line_to(s * 0.03, -s * 0.18);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(c(0xfff8ee)));
    }
    let mut pb = PathBuilder::new();
    pb.move_to(-s * 0.03, -s * 0.8);
    pb.line_to(-s * 0.03, -s * 0.18);
    pb.line_to(-s * 0.38, -s * 0.18);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(c(0xe8590c)));
    }
}

/// Diều (gốc (0,0) = tâm diều, cỡ s).
pub fn kite(cv: &mut Canvas, s: f32, a: Color, b: Color) {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, -s);
    pb.line_to(s * 0.7, 0.0);
    pb.line_to(0.0, s * 1.2);
    pb.line_to(-s * 0.7, 0.0);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(a));
    }
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, -s);
    pb.line_to(s * 0.7, 0.0);
    pb.line_to(0.0, 0.0);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(b));
    }
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0);
    pb.line_to(-s * 0.7, 0.0);
    pb.line_to(0.0, s * 1.2);
    pb.close();
    if let Some(p) = pb.finish() {
        cv.fill(&p, solid(b));
    }
    // đuôi
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, s * 1.2);
    for k in 1..6 {
        let k = k as f32;
        pb.line_to((k * 1.3).sin() * s * 0.25, s * 1.2 + k * s * 0.35);
    }
    if let Some(p) = pb.finish() {
        cv.stroke(&p, solid(a), s * 0.06);
    }
}

/// Đèn ông sao (gốc = tâm ngôi sao, cỡ s).
pub fn star_lantern(cv: &mut Canvas, s: f32) {
    cv.glow(0.0, 0.0, s * 3.2, (255, 190, 90), 0.5);
    if let Some(p) = star_path(0.0, 0.0, s, s * 0.45, 5) {
        cv.fill(&p, rad(0.0, 0.0, 0.0, 0.0, s, &[(0.0, c(0xfff3c4)), (0.6, c(0xff9a3c)), (1.0, c(0xe8261d))]));
        cv.stroke(&p, solid(c(0xffd27a)), s * 0.06);
    }
    for k in 0..3 {
        cv.line(-s * 0.2 + k as f32 * s * 0.2, s * 0.6, -s * 0.25 + k as f32 * s * 0.25, s * 1.1, solid(c(0xffd27a)), s * 0.04);
    }
}

// ======================================================================== phong cách minh hoạ (chibi)

/// Kiểu tóc của nhân vật minh hoạ.
#[derive(Clone, Copy, PartialEq)]
pub enum Hair {
    /// tóc ngắn (bé trai, ông)
    Short,
    /// búi tóc (mẹ, bà)
    Bun,
    /// tóc dài xoã (chị, cô giáo)
    Long,
    /// tóc hai chỏm (bé gái)
    Pigtails,
}

/// Trang phục: màu áo, màu quần, kiểu tóc, màu tóc, băng đô (khăn) nếu có.
#[derive(Clone, Copy)]
pub struct Outfit {
    pub shirt: u32,
    pub pants: u32,
    pub hair: Hair,
    pub hair_col: u32,
    pub band: Option<u32>,
    /// áo dài (hai tà dài) thay cho áo ngắn
    pub aodai: bool,
    /// màu hoạ tiết hoa trên áo dài
    pub pattern: u32,
}

const SKIN: u32 = 0xffdcc0;
const SKIN_SH: u32 = 0xf2b994;

fn shade(hex: u32, k: f32) -> Color {
    let f = |sh: u32| (((hex >> sh) & 0xff) as f32 * k).clamp(0.0, 255.0) as u8;
    ca(f(16), f(8), f(0), 1.0)
}

/// Màu nét viền (nâu đậm, như nét đen in sau cùng của tranh Đông Hồ / hoạt hình phẳng).
const LINE: u32 = 0x3b2416;

fn opath(cv: &mut Canvas, p: &tiny_skia::Path, fill: Color, lw: f32) {
    cv.fill(p, solid(fill));
    cv.stroke(p, solid(c(LINE)), lw);
}

/// Tay/chân có viền: nét đậm màu viền rồi nét màu áo đè lên.
fn olimb(cv: &mut Canvas, pts: &[(f32, f32)], w: f32, col: Color, lw: f32) {
    limb(cv, pts, w + lw * 2.0, c(LINE));
    limb(cv, pts, w, col);
}

fn ocircle(cv: &mut Canvas, x: f32, y: f32, r: f32, col: Color, lw: f32) {
    cv.circle(x, y, r + lw, solid(c(LINE)));
    cv.circle(x, y, r, solid(col));
}

/// Người vẽ theo tỉ lệ thật (người lớn ~6,5 đầu, trẻ em ~4,5 đầu), khung xương có khớp
/// (vai – khuỷu – cổ tay, hông – gối – cổ chân), phong cách hoạt hình phẳng có nét viền.
/// Cao `h`, chân chạm đất ở (x, ground), quay mặt theo `dir` (1 = phải, -1 = trái).
/// Áo dài (`o.aodai`): thân ôm eo, hai tà dài qua gối, quần lụa; khăn vấn nếu `o.band`.
/// Trả về vị trí bàn tay phía trước (để cầm cờ, đèn, hoa, diều...).
pub fn chibi(cv: &mut Canvas, x: f32, ground: f32, h: f32, pose: Pose, dir: f32, o: Outfit) -> (f32, f32) {
    let d = dir;
    let kid = h < 100.0;
    let lw = (h * 0.011).max(1.0);
    let r = if kid { h * 0.105 } else { h * 0.072 };
    let thigh = h * if kid { 0.21 } else { 0.245 };
    let shin = h * if kid { 0.2 } else { 0.235 };
    let torso = h * if kid { 0.24 } else { 0.29 };
    let (sh_w, waist_w, hip_w) = if kid { (h * 0.1, h * 0.085, h * 0.09) } else { (h * 0.115, h * 0.07, h * 0.09) };
    let (ua, fa) = (h * if kid { 0.14 } else { 0.165 }, h * if kid { 0.13 } else { 0.15 });
    let (arm_w, leg_w) = (h * if kid { 0.055 } else { 0.045 }, h * if kid { 0.07 } else { 0.062 });
    let skin = c(SKIN);
    let skin_d = c(SKIN_SH);
    let shirt = c(o.shirt);
    let shirt_d = shade(o.shirt, 0.8);
    let pants = c(o.pants);
    let shoe = c(0x4a3226);
    let dirv = |a: f32| (d * a.sin(), a.cos());

    // ---- khung xương theo tư thế
    let kneel = pose == Pose::Sit;
    let squat = pose == Pose::Squat;
    let hip_y = if kneel { ground - h * 0.16 } else if squat { ground - h * 0.2 } else { ground - thigh - shin - h * 0.02 };
    let lean = if kneel { d * h * 0.03 } else if squat { d * h * 0.07 } else { 0.0 };
    let sh_y = hip_y - torso;
    let neck_top = sh_y - h * 0.03;
    let head = (x + lean * 1.4, neck_top - r * 0.92);
    let waist_y = hip_y - torso * 0.32;

    // chân: (hông, gối, cổ chân) cho chân sau và chân trước
    let legs: [((f32, f32), (f32, f32), (f32, f32)); 2] = if kneel {
        let k = (x + d * thigh * 0.8, ground - h * 0.035);
        let f = (x - d * h * 0.06, ground - h * 0.025);
        [((x - d * hip_w * 0.2, hip_y), (k.0 - d * h * 0.03, k.1), (f.0 - d * h * 0.02, f.1)), ((x + d * hip_w * 0.2, hip_y), k, f)]
    } else if squat {
        let k = (x + d * h * 0.14, ground - h * 0.23);
        let f = (x + d * h * 0.1, ground - h * 0.025);
        [((x - d * hip_w * 0.2, hip_y), (k.0 - d * h * 0.04, k.1 + h * 0.01), (f.0 - d * h * 0.05, f.1)), ((x + d * hip_w * 0.2, hip_y), k, f)]
    } else {
        let step = if pose == Pose::Walk { h * 0.09 } else { h * 0.012 };
        let mk = |s: f32| {
            let hp = (x + s * hip_w * 0.45, hip_y);
            let k = (hp.0 + d * s * step * 0.5, hip_y + thigh);
            let f = (hp.0 + d * s * step, ground - h * 0.025);
            (hp, k, f)
        };
        [mk(-1.0), mk(1.0)]
    };

    // tay: góc (rad, 0 = buông xuống, PI/2 = đưa ra trước, PI = giơ lên) cho cánh tay và cẳng tay
    let (back_a, front_a): ((f32, f32), (f32, f32)) = match pose {
        Pose::Stand => ((-0.08, 0.05), (0.12, 0.3)),
        Pose::Walk => ((-0.4, -0.2), (0.4, 0.75)),
        Pose::Sit => ((0.55, 1.25), (0.75, 1.45)),
        Pose::Squat => ((0.7, 0.35), (1.0, 1.5)),
        Pose::RaiseHand => ((-0.08, 0.05), (2.6, 2.95)),
        Pose::Give => ((-0.05, 0.1), (1.15, 1.55)),
        Pose::Cheer => ((-2.5, -2.85), (2.5, 2.85)),
    };
    let sh_back = (x - d * sh_w * 0.75 + lean, sh_y + h * 0.015);
    let sh_front = (x + d * sh_w * 0.75 + lean, sh_y + h * 0.015);
    let arm = |s: (f32, f32), a: (f32, f32)| {
        let e = (s.0 + ua * dirv(a.0).0, s.1 + ua * dirv(a.0).1);
        let hn = (e.0 + fa * dirv(a.1).0, e.1 + fa * dirv(a.1).1);
        (e, hn)
    };
    let (eb, hb) = arm(sh_back, back_a);
    let (ef, hf) = arm(sh_front, front_a);

    // ---- bóng đổ
    cv.ellipse(x + h * 0.04, ground + h * 0.004, h * if kneel || squat { 0.24 } else { 0.17 }, h * 0.03, 0.0, solid(ca(0, 0, 0, 0.16)));

    // ---- tóc dài phía sau (sau thân)
    let hair = c(o.hair_col);
    if o.hair == Hair::Long {
        let mut pb = PathBuilder::new();
        pb.move_to(head.0 - r * 1.0, head.1 - r * 0.2);
        pb.quad_to(head.0 - d * r * 1.3, sh_y + h * 0.1, head.0 - d * r * 0.9, sh_y + h * 0.2);
        pb.line_to(head.0 + d * r * 0.3, sh_y + h * 0.17);
        pb.quad_to(head.0 + r * 1.05, head.1 + r * 0.6, head.0 + r * 1.0, head.1 - r * 0.2);
        pb.close();
        if let Some(p) = pb.finish() {
            opath(cv, &p, hair, lw);
        }
    }

    // ---- tay sau (tay áo tối hơn)
    olimb(cv, &[sh_back, eb, hb], arm_w, shirt_d, lw);
    ocircle(cv, hb.0, hb.1, arm_w * 0.55, skin_d, lw);

    // ---- tà sau áo dài
    if o.aodai && !kneel && !squat {
        let mut pb = PathBuilder::new();
        pb.move_to(x - waist_w, waist_y);
        pb.line_to(x + waist_w * 0.2, waist_y);
        pb.line_to(x - d * hip_w * 0.1, ground - h * 0.12);
        pb.quad_to(x - d * hip_w * 1.2, ground - h * 0.11, x - d * hip_w * 1.35, ground - h * 0.14);
        pb.close();
        if let Some(p) = pb.finish() {
            opath(cv, &p, shirt_d, lw);
        }
    }

    // ---- chân (quần) + giày
    for (i, (hp, k, f)) in legs.iter().enumerate() {
        let col = if i == 0 { shade(o.pants, 0.88) } else { pants };
        olimb(cv, &[*hp, *k, *f], leg_w, col, lw);
        if let Some(p) = ellipse_path(f.0 + d * h * 0.025, f.1 + h * 0.005, h * 0.045, h * 0.022, 0.0) {
            opath(cv, &p, shoe, lw);
        }
    }

    // ---- thân áo: vai – eo – hông
    let hem_y = if o.aodai { waist_y + h * 0.02 } else { hip_y + h * 0.03 };
    let mut pb = PathBuilder::new();
    pb.move_to(x - sh_w + lean, sh_y);
    pb.quad_to(x + lean, sh_y - h * 0.015, x + sh_w + lean, sh_y);
    pb.quad_to(x + sh_w * 0.95 + lean * 0.5, (sh_y + waist_y) / 2.0, x + waist_w, waist_y);
    pb.line_to(x + hip_w * if o.aodai { 0.9 } else { 1.0 }, hem_y);
    pb.line_to(x - hip_w * if o.aodai { 0.9 } else { 1.0 }, hem_y);
    pb.line_to(x - waist_w, waist_y);
    pb.quad_to(x - sh_w * 0.95 + lean * 0.5, (sh_y + waist_y) / 2.0, x - sh_w + lean, sh_y);
    pb.close();
    if let Some(p) = pb.finish() {
        opath(cv, &p, shirt, lw);
    }
    // nếp áo và bóng một bên
    cv.line(x - d * sh_w * 0.5 + lean, sh_y + h * 0.04, x - d * waist_w * 0.6, waist_y - h * 0.01, solid(ca(0, 0, 0, 0.12)), lw);
    // tà trước áo dài (thả dài qua gối)
    if o.aodai {
        let mut pb = PathBuilder::new();
        pb.move_to(x - waist_w * 0.85, waist_y + h * 0.01);
        pb.line_to(x + waist_w * 0.85, waist_y + h * 0.01);
        if kneel || squat {
            pb.quad_to(x + d * h * 0.2, ground - h * 0.03, x + d * h * 0.22, ground - h * 0.01);
            pb.line_to(x - d * h * 0.02, ground - h * 0.01);
        } else {
            pb.line_to(x + d * hip_w * 1.1, ground - h * 0.13);
            pb.quad_to(x + d * hip_w * 0.4, ground - h * 0.11, x - d * hip_w * 0.3, ground - h * 0.13);
        }
        pb.close();
        if let Some(p) = pb.finish() {
            opath(cv, &p, shirt, lw);
        }
        // đường cài chéo từ cổ sang nách, hoạ tiết hoa nhỏ
        cv.line(x + lean, sh_y + h * 0.005, x - d * sh_w * 0.7 + lean, sh_y + h * 0.07, solid(c(LINE)), lw * 0.8);
        let pc = c(o.pattern);
        for (dx, fy) in [(-0.5f32, 0.06f32), (0.45, 0.12), (-0.1, 0.2), (0.4, 0.34), (-0.3, 0.45), (0.2, 0.58)] {
            let py = sh_y + fy * (ground - sh_y);
            let px = x + dx * hip_w + lean * 0.4;
            if py < ground - h * 0.13 || (kneel && py < ground - h * 0.04) {
                for k in 0..5 {
                    let a = k as f32 * std::f32::consts::TAU / 5.0;
                    cv.circle(px + a.cos() * h * 0.009, py + a.sin() * h * 0.009, h * 0.0075, solid(pc));
                }
            }
        }
    } else {
        // hàng cúc áo
        for k in 0..3 {
            cv.circle(x + lean * 0.6, sh_y + h * (0.05 + k as f32 * 0.06), h * 0.008, solid(shade(o.shirt, 0.6)));
        }
    }
    // cổ áo
    if let Some(p) = round_rect_path(x - h * 0.03 + lean, sh_y - h * 0.022, h * 0.06, h * 0.03, h * 0.01) {
        opath(cv, &p, shirt_d, lw * 0.8);
    }

    // ---- cổ và đầu
    cv.rect(head.0 - r * 0.28, head.1 + r * 0.7, r * 0.56, sh_y - head.1 - r * 0.6, solid(skin_d));
    let (hx, hy) = head;
    // tai (phía sau mặt)
    ocircle(cv, hx - d * r * 0.78, hy + r * 0.12, r * 0.2, skin, lw);
    if let Some(p) = ellipse_path(hx, hy, r * 0.9, r, 0.0) {
        opath(cv, &p, skin, lw);
    }
    // tóc trước: mái + ngôi rẽ
    let mut pb = PathBuilder::new();
    pb.move_to(hx - r * 0.95, hy + r * 0.05);
    pb.cubic_to(hx - r * 1.05, hy - r * 1.3, hx + r * 1.05, hy - r * 1.3, hx + r * 0.95, hy + r * 0.05);
    if o.hair == Hair::Short {
        pb.quad_to(hx + r * 0.85, hy - r * 0.5, hx + d * r * 0.35, hy - r * 0.55);
        pb.quad_to(hx - d * r * 0.1, hy - r * 0.35, hx - r * 0.5 * d, hy - r * 0.5);
        pb.quad_to(hx - r * 0.85, hy - r * 0.45, hx - r * 0.95, hy + r * 0.05);
    } else {
        pb.quad_to(hx + r * 0.8, hy - r * 0.4, hx + d * r * 0.15, hy - r * 0.62);
        pb.quad_to(hx - r * 0.7, hy - r * 0.45, hx - r * 0.95, hy + r * 0.05);
    }
    pb.close();
    if let Some(p) = pb.finish() {
        opath(cv, &p, hair, lw);
    }
    cv.line(hx - r * 0.5, hy - r * 0.75, hx + r * 0.15, hy - r * 0.92, solid(ca(255, 255, 255, 0.25)), r * 0.08);
    match o.hair {
        Hair::Bun => ocircle(cv, hx - d * r * 0.7, hy - r * 0.75, r * 0.38, hair, lw),
        Hair::Pigtails => {
            for s in [-1.0f32, 1.0] {
                olimb(cv, &[(hx + s * r * 0.85, hy - r * 0.2), (hx + s * r * 1.05, hy + r * 0.5), (hx + s * r * 0.95, hy + r * 1.0)], r * 0.32, hair, lw);
                cv.circle(hx + s * r * 0.88, hy - r * 0.15, r * 0.1, solid(c(0xff6b9a)));
            }
        }
        _ => {}
    }
    if let Some(b) = o.band {
        if matches!(o.hair, Hair::Long | Hair::Bun) {
            // khăn vấn
            if let Some(p) = ellipse_path(hx, hy - r * 0.6, r * 1.02, r * 0.34, 0.0) {
                opath(cv, &p, c(b), lw);
            }
            for k in 0..5 {
                let kx = hx - r * 0.75 + k as f32 * r * 0.38;
                cv.line(kx - r * 0.1, hy - r * 0.82, kx + r * 0.1, hy - r * 0.4, solid(shade(b, 0.75)), lw * 0.9);
            }
        } else {
            let mut pb = PathBuilder::new();
            pb.move_to(hx - r * 0.95, hy - r * 0.3);
            pb.quad_to(hx, hy - r * 0.62, hx + r * 0.95, hy - r * 0.3);
            if let Some(p) = pb.finish() {
                cv.stroke(&p, solid(c(LINE)), r * 0.18 + lw * 2.0);
                cv.stroke(&p, solid(c(b)), r * 0.18);
            }
        }
    }
    // mặt (hơi nghiêng 3/4 theo hướng nhìn)
    let fx = hx + d * r * 0.12;
    for s in [-1.0f32, 1.0] {
        let ex = fx + s * r * 0.36;
        let ey = hy + r * 0.08;
        cv.ellipse(ex, ey, r * 0.17, r * 0.11, 0.0, solid(c(0xffffff)));
        cv.circle(ex + d * r * 0.03, ey, r * 0.09, solid(c(0x4a2e1e)));
        cv.circle(ex + d * r * 0.03, ey, r * 0.045, solid(c(0x120a06)));
        cv.circle(ex + d * r * 0.06, ey - r * 0.04, r * 0.028, solid(c(0xffffff)));
        let mut pb = PathBuilder::new();
        pb.move_to(ex - r * 0.18, ey + r * 0.01);
        pb.quad_to(ex, ey - r * 0.16, ex + r * 0.18, ey - r * 0.01);
        if let Some(p) = pb.finish() {
            cv.stroke(&p, solid(c(LINE)), lw * 1.2);
        }
        cv.line(ex - r * 0.15, ey - r * 0.27, ex + r * 0.15, ey - r * 0.3, solid(shade(o.hair_col, 0.85)), lw * 1.1);
        cv.circle(ex + s * r * 0.05, hy + r * 0.45, r * 0.12, solid(ca(255, 120, 120, 0.3)));
    }
    // mũi
    cv.line(fx + d * r * 0.04, hy + r * 0.2, fx + d * r * 0.1, hy + r * 0.36, solid(c(0xd99a7a)), lw);
    // miệng cười
    let mut pb = PathBuilder::new();
    let mw = r * if kid { 0.2 } else { 0.17 };
    pb.move_to(fx - mw, hy + r * 0.55);
    pb.quad_to(fx, hy + r * if kid { 0.82 } else { 0.72 }, fx + mw, hy + r * 0.55);
    pb.close();
    if let Some(p) = pb.finish() {
        opath(cv, &p, c(0xc2414e), lw * 0.7);
    }

    // ---- tay trước + bàn tay
    olimb(cv, &[sh_front, ef, hf], arm_w, shirt, lw);
    ocircle(cv, hf.0, hf.1, arm_w * 0.55, skin, lw);
    hf
}

/// Con lân kiểu minh hoạ (đầu tròn to, viền lông trắng, mắt to, sừng xoắn, ngọc xanh),
/// đầu quay sang trái, gốc (0,0) = tâm đầu, cỡ s = bán kính đầu.
pub fn lion_head_art(cv: &mut Canvas, s: f32) {
    let red = c(0xe03131);
    let red_d = c(0xb01e1e);
    let gold = c(0xf5b82e);
    let green = c(0x2f9e44);
    let white = c(0xffffff);
    // tai
    for (ex, rot) in [(-0.55f32, -0.5f32), (0.62, 0.45)] {
        let mut pb = PathBuilder::new();
        pb.move_to(ex * s - s * 0.25, -s * 0.55);
        pb.line_to(ex * s + rot * s * 0.4, -s * 1.15);
        pb.line_to(ex * s + s * 0.28, -s * 0.6);
        pb.close();
        if let Some(p) = pb.finish() {
            cv.fill(&p, solid(red));
            cv.stroke(&p, solid(white), s * 0.08);
        }
    }
    // khối đầu đỏ
    cv.circle(0.0, 0.0, s, rad(-s * 0.35, -s * 0.4, 0.0, 0.0, s * 1.2, &[(0.0, c(0xff6b6b)), (0.6, red), (1.0, red_d)]));
    // viền lông trắng bông quanh mặt
    for i in 0..22 {
        let a = PI * 0.15 + i as f32 * PI * 1.7 / 21.0;
        cv.circle(a.cos() * s * 0.98, a.sin() * s * 0.98, s * 0.2, solid(white));
    }
    // trán: dải vàng + ngọc xanh
    cv.ellipse(0.0, -s * 0.52, s * 0.55, s * 0.13, 0.0, solid(gold));
    cv.circle(0.0, -s * 0.55, s * 0.17, solid(gold));
    cv.circle(0.0, -s * 0.55, s * 0.12, rad(-s * 0.04, -s * 0.6, 0.0, -s * 0.55, s * 0.12, &[(0.0, c(0x8ce99a)), (1.0, green)]));
    for k in [-1.0f32, 1.0] {
        cv.circle(k * s * 0.38, -s * 0.5, s * 0.07, solid(green));
        cv.circle(k * s * 0.38, -s * 0.5, s * 0.035, solid(c(0xb2f2bb)));
    }
    // sừng xoắn xanh trắng
    for k in 0..4 {
        let w = s * (0.22 - k as f32 * 0.045);
        let y = -s * (0.9 + k as f32 * 0.15);
        cv.ellipse(0.0, y, w, s * 0.08, 0.0, solid(if k % 2 == 0 { green } else { white }));
    }
    // mắt to: mí xanh, tròng nâu, đồng tử đen, điểm sáng
    for ex in [-0.4f32, 0.32] {
        let (cx, cy) = (ex * s, -s * 0.18);
        cv.circle(cx, cy, s * 0.26, solid(gold));
        cv.circle(cx, cy, s * 0.22, solid(white));
        cv.circle(cx - s * 0.03, cy + s * 0.02, s * 0.15, rad(cx, cy, cx, cy, s * 0.15, &[(0.0, c(0xa86c3a)), (1.0, c(0x5a3010))]));
        cv.circle(cx - s * 0.03, cy + s * 0.02, s * 0.08, solid(c(0x111111)));
        cv.circle(cx + s * 0.03, cy - s * 0.04, s * 0.04, solid(white));
        let mut pb = PathBuilder::new();
        pb.move_to(cx - s * 0.26, cy - s * 0.06);
        pb.quad_to(cx, cy - s * 0.36, cx + s * 0.26, cy - s * 0.06);
        if let Some(p) = pb.finish() {
            cv.stroke(&p, solid(green), s * 0.08);
        }
    }
    // mũi tròn vàng
    cv.circle(-s * 0.05, s * 0.12, s * 0.14, solid(gold));
    cv.circle(-s * 0.08, s * 0.08, s * 0.05, solid(c(0xfff3bf)));
    // miệng há: viền trắng, trong đỏ sẫm, lưỡi hồng, hàm răng
    cv.ellipse(-s * 0.05, s * 0.5, s * 0.62, s * 0.3, 0.0, solid(white));
    cv.ellipse(-s * 0.05, s * 0.52, s * 0.52, s * 0.22, 0.0, solid(c(0x8b1a1a)));
    cv.ellipse(-s * 0.05, s * 0.62, s * 0.3, s * 0.1, 0.0, solid(c(0xff8787)));
    for k in 0..7 {
        cv.rect(-s * 0.5 + k as f32 * s * 0.13, s * 0.34, s * 0.08, s * 0.07, solid(white));
    }
    // chấm vàng trang trí hai bên má
    for (dx, dy) in [(-0.78f32, 0.2f32), (0.72, 0.18), (-0.7, -0.45), (0.68, -0.45)] {
        cv.circle(dx * s, dy * s, s * 0.07, solid(gold));
    }
}

/// Thân lân kiểu minh hoạ: lớp vảy xanh–đỏ–trắng viền vàng, chân lông trắng, đuôi bông.
/// Gốc (0,0) = chỗ nối với đầu (bên trái), thân kéo sang phải, dài khoảng 2.4 s.
pub fn lion_body_art(cv: &mut Canvas, s: f32) {
    let red = c(0xe03131);
    let gold = c(0xf5b82e);
    let white = c(0xffffff);
    // chân (sau trước)
    for (lx, front) in [(s * 1.95, false), (s * 1.6, false), (s * 0.55, true), (s * 0.2, true)] {
        let col = if front { white } else { c(0xe9ecef) };
        // chân lông xù: các vòng tròn trắng chồng nhau, vòng vàng, bàn chân
        for k in 0..3 {
            cv.circle(lx, s * (0.32 + k as f32 * 0.18), s * 0.2, solid(col));
        }
        cv.round_rect(lx - s * 0.2, s * 0.52, s * 0.4, s * 0.07, s * 0.035, solid(gold));
        cv.ellipse(lx - s * 0.04, s * 0.88, s * 0.25, s * 0.11, 0.0, solid(col));
    }
    // thân đỏ (dày, uốn cong như đang nhún)
    cv.round_rect(0.0, -s * 0.62, s * 2.3, s * 1.05, s * 0.5, lin(0.0, -s * 0.62, 0.0, s * 0.43, &[(0.0, c(0xff6b6b)), (1.0, c(0xb01e1e))]));
    // các lớp vảy (vòng cung) từ đầu tới đuôi
    let rows = [(c(0x2f9e44), -0.42f32), (white, -0.16), (c(0x2f9e44), 0.06), (red, 0.26)];
    for (ri, (col, dy)) in rows.iter().enumerate() {
        let n = 6;
        for k in 0..n {
            let cx = s * 0.25 + k as f32 * s * 0.36 + ri as f32 * s * 0.12;
            let cy = dy * s;
            let mut pb = PathBuilder::new();
            pb.move_to(cx - s * 0.2, cy);
            pb.quad_to(cx, cy + s * 0.32, cx + s * 0.2, cy);
            pb.close();
            if let Some(p) = pb.finish() {
                cv.fill(&p, solid(*col));
                cv.stroke(&p, solid(gold), s * 0.035);
            }
        }
    }
    // tua viền dưới
    for k in 0..12 {
        cv.rect(s * 0.1 + k as f32 * s * 0.18, s * 0.36, s * 0.06, s * 0.16, solid(gold));
    }
    // đuôi bông trắng
    for (dx, dy, rr) in [(2.35f32, -0.45f32, 0.2f32), (2.5, -0.62, 0.17), (2.62, -0.8, 0.13)] {
        cv.circle(dx * s, dy * s, rr * s, solid(white));
    }
}

/// Nồi bánh chưng kiểu minh hoạ: nồi nhôm có quai, bánh chưng chất trên miệng nồi,
/// củi có vân gỗ, lửa nhiều lớp. Gốc = giữa đáy bếp trên mặt đất, cỡ s (bề rộng nồi ~1.4 s).
pub fn pot_art(cv: &mut Canvas, x: f32, ground: f32, s: f32) {
    // củi chụm
    for (dx, a) in [(-0.6f32, -0.5f32), (-0.2, -0.15), (0.25, 0.2), (0.6, 0.55)] {
        cv.ellipse(x + dx * s, ground - s * 0.1, s * 0.5, s * 0.1, a, solid(c(0x7a4a26)));
        let ex = x + dx * s + a.cos() * s * 0.48 * dx.signum();
        cv.circle(ex, ground - s * 0.1 + a.sin() * s * 0.48 * dx.signum(), s * 0.09, solid(c(0xc08a5a)));
    }
    // lửa (lớp tĩnh, cảnh vẽ thêm lửa động lên trên)
    for (dx, hh, col) in [(-0.4f32, 0.55f32, 0xe8401c), (0.0, 0.75, 0xff7a1a), (0.38, 0.5, 0xe8401c), (0.0, 0.45, 0xffd43b)] {
        let mut pb = PathBuilder::new();
        let bx = x + dx * s;
        pb.move_to(bx - s * 0.22, ground - s * 0.12);
        pb.quad_to(bx - s * 0.2, ground - s * (0.12 + hh * 0.6), bx, ground - s * (0.12 + hh));
        pb.quad_to(bx + s * 0.2, ground - s * (0.12 + hh * 0.6), bx + s * 0.22, ground - s * 0.12);
        pb.close();
        if let Some(p) = pb.finish() {
            cv.fill(&p, solid(c(col)));
        }
    }
    // nồi nhôm
    let (pw, ph) = (s * 1.4, s * 1.1);
    let top = ground - s * 0.55 - ph;
    let grey = lin(x - pw / 2.0, 0.0, x + pw / 2.0, 0.0, &[(0.0, c(0x8f959e)), (0.3, c(0xdde1e6)), (0.55, c(0xc3c8cf)), (1.0, c(0x7c828b))]);
    cv.round_rect(x - pw / 2.0, top, pw, ph, s * 0.08, grey);
    for k in [-1.0f32, 1.0] {
        let mut pb = PathBuilder::new();
        pb.move_to(x + k * pw / 2.0, top + s * 0.12);
        pb.quad_to(x + k * (pw / 2.0 + s * 0.22), top + s * 0.14, x + k * pw / 2.0, top + s * 0.36);
        if let Some(p) = pb.finish() {
            cv.stroke(&p, solid(c(0x8f959e)), s * 0.07);
        }
    }
    cv.ellipse(x, top, pw / 2.0, s * 0.12, 0.0, solid(c(0xe9ecef)));
    // bánh chưng chất trên miệng nồi
    for (dx, dy, rot) in [(-0.38f32, -0.12f32, -0.2f32), (0.0, -0.2, 0.05), (0.36, -0.1, 0.25), (-0.15, -0.4, -0.1), (0.2, -0.42, 0.15)] {
        let (bx, by) = (x + dx * s, top + dy * s);
        let w = s * 0.42;
        let t = tiny_skia::Transform::from_rotate_at(rot.to_degrees(), bx, by);
        let rect = PathBuilder::from_rect(tiny_skia::Rect::from_xywh(bx - w / 2.0, by - w / 2.0, w, w).unwrap());
        cv.fill_t(&rect, lin(bx - w / 2.0, by - w / 2.0, bx + w / 2.0, by + w / 2.0, &[(0.0, c(0x69db7c)), (1.0, c(0x2b8a3e))]), t);
        for k in [-0.17f32, 0.17] {
            let l1 = PathBuilder::from_rect(tiny_skia::Rect::from_xywh(bx + k * w - 1.0, by - w / 2.0, 2.0, w).unwrap());
            let l2 = PathBuilder::from_rect(tiny_skia::Rect::from_xywh(bx - w / 2.0, by + k * w - 1.0, w, 2.0).unwrap());
            cv.fill_t(&l1, solid(c(0xfff3bf)), t);
            cv.fill_t(&l2, solid(c(0xfff3bf)), t);
        }
    }
}
