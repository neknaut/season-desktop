//! Danh sách lệnh vẽ của một khung hình: các "sprite" gửi lên GPU theo thứ tự.

pub const W: f32 = 1280.0;
pub const H: f32 = 720.0;
pub const TAU: f32 = std::f32::consts::TAU;

pub const GLOW: u32 = 0;
pub const DISC: u32 = 1;
pub const TEXTURE: u32 = 3;
pub const RECT: u32 = 4;
pub const RAY: u32 = 5;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Sprite {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub rot: f32,
    pub kind: u32,
    pub color: [f32; 4],
    pub uv: [f32; 4],
}

/// Cách trộn màu: Alpha = vẽ đè bình thường, Add = cộng sáng (như 'lighter' trong canvas).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blend {
    Alpha,
    Add,
}

/// Texture dùng cho lệnh vẽ.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tex {
    Atlas = 0,
    Back = 1,
    Front = 2,
    Widget = 3,
}

pub struct Cmd {
    pub blend: Blend,
    pub tex: Tex,
    pub start: u32,
    pub end: u32,
}

#[derive(Default)]
pub struct DrawList {
    pub sprites: Vec<Sprite>,
    pub cmds: Vec<Cmd>,
    /// Vùng màn hình tính theo toạ độ thiết kế: tâm x, tâm y, rộng, cao.
    pub screen: [f32; 4],
}

impl DrawList {
    pub fn clear(&mut self) {
        self.sprites.clear();
        self.cmds.clear();
    }

    pub fn push(&mut self, blend: Blend, tex: Tex, s: Sprite) {
        let n = self.sprites.len() as u32;
        match self.cmds.last_mut() {
            Some(c) if c.blend == blend && c.tex == tex && c.end == n => c.end += 1,
            _ => self.cmds.push(Cmd { blend, tex, start: n, end: n + 1 }),
        }
        self.sprites.push(s);
    }

    fn sprite(kind: u32, cx: f32, cy: f32, w: f32, h: f32, rot: f32, rgb: [f32; 3], a: f32, uv: [f32; 4]) -> Sprite {
        Sprite { pos: [cx, cy], size: [w, h], rot, kind, color: [rgb[0], rgb[1], rgb[2], a.clamp(0.0, 1.0)], uv }
    }

    /// Lớp ảnh phủ cả màn hình (nền tĩnh, tiền cảnh tĩnh).
    pub fn layer(&mut self, tex: Tex) {
        let [cx, cy, w, h] = self.screen;
        self.push(Blend::Alpha, tex, Self::sprite(TEXTURE, cx, cy, w, h, 0.0, [1.0; 3], 1.0, [0.0, 0.0, 1.0, 1.0]));
    }

    /// Ảnh đặt tại một vùng (toạ độ thiết kế, góc trên trái + kích thước).
    pub fn image(&mut self, tex: Tex, x: f32, y: f32, w: f32, h: f32) {
        self.push(Blend::Alpha, tex, Self::sprite(TEXTURE, x + w / 2.0, y + h / 2.0, w, h, 0.0, [1.0; 3], 1.0, [0.0, 0.0, 1.0, 1.0]));
    }

    /// Quầng sáng tròn mờ dần, tâm (x, y), đường kính size (có thể kéo dãn w x h).
    pub fn glow(&mut self, blend: Blend, x: f32, y: f32, w: f32, h: f32, rgb: [f32; 3], a: f32) {
        if a > 0.002 {
            self.push(blend, Tex::Atlas, Self::sprite(GLOW, x, y, w, h, 0.0, rgb, a, [0.0; 4]));
        }
    }

    pub fn rect(&mut self, blend: Blend, cx: f32, cy: f32, w: f32, h: f32, rot: f32, rgb: [f32; 3], a: f32) {
        if a > 0.002 {
            self.push(blend, Tex::Atlas, Self::sprite(RECT, cx, cy, w, h, rot, rgb, a, [0.0; 4]));
        }
    }

    /// Tia sáng mềm từ (x, y) toả theo góc `rot`, dài `len`, ngọn rộng `w`.
    pub fn ray(&mut self, x: f32, y: f32, len: f32, w: f32, rot: f32, rgb: [f32; 3], a: f32) {
        if a > 0.002 {
            let (c, s) = (rot.cos(), rot.sin());
            self.push(Blend::Add, Tex::Atlas, Self::sprite(RAY, x + c * len * 0.5, y + s * len * 0.5, len, w, rot, rgb, a, [0.0; 4]));
        }
    }

    pub fn disc(&mut self, blend: Blend, cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, rgb: [f32; 3], a: f32) {
        if a > 0.002 {
            self.push(blend, Tex::Atlas, Self::sprite(DISC, cx, cy, rx * 2.0, ry * 2.0, rot, rgb, a, [0.0; 4]));
        }
    }

    /// Ảnh trong atlas, tâm (cx, cy), xoay quanh tâm.
    pub fn atlas(&mut self, uv: [f32; 4], cx: f32, cy: f32, w: f32, h: f32, rot: f32, a: f32) {
        if a > 0.002 {
            self.push(Blend::Alpha, Tex::Atlas, Self::sprite(TEXTURE, cx, cy, w, h, rot, [1.0; 3], a, uv));
        }
    }
}

/// Màu từ mã hex 0xRRGGBB.
pub const fn hex(h: u32) -> [f32; 3] {
    [((h >> 16) & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, (h & 255) as f32 / 255.0]
}

pub fn rgb8(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// Bộ sinh số ngẫu nhiên giống hệt mulberry32 trong bản xem thử, để cảnh vẽ ra y như vậy.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }
    pub fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64 / 4_294_967_296.0) as f32
    }
}

/// Số ngẫu nhiên nhanh cho hạt bay (thay Math.random()).
pub struct Fast(u32);

impl Fast {
    pub fn seeded() -> Self {
        let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(1);
        Self(s | 1)
    }
    pub fn f(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / 16_777_216.0
    }
}
