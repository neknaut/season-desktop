//! Các cảnh theo mùa, chuyển từ bản xem thử (canvas) sang Rust.
//! Mỗi cảnh có: lớp nền tĩnh (back), lớp tiền cảnh tĩnh (front), ảnh nhỏ trong atlas,
//! và phần chuyển động vẽ lại mỗi khung hình.

mod festive;
mod landscape;
mod foreground;
mod depth;
mod particles;
mod painting;
pub(crate) use painting::thumbnail;
pub mod figures;
mod moon;
mod noel;
mod tet;

use tiny_skia::Transform;

use crate::config::Config;
use crate::draw::{DrawList, Tex, H, W};
use crate::paint::{Atlas, Canvas};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Noel,
    Tet,
    Moon,
    // bốn mùa (ngày thường)
    Spring,
    Summer,
    Autumn,
    Winter,
    // ngày lễ
    NewYear,
    Valentine,
    Women,
    Reunify,
    Children,
    National,
    VnWomen,
    Halloween,
    Teachers,
    HungKings,
    DoanNgo,
    Remembrance,
    CultureDay,
}

/// Màu và chữ riêng của từng chủ đề (taskbar, đồng hồ, cửa sổ cài đặt).
pub struct Theme {
    /// nền kính taskbar (r, g, b, độ đậm) và viền
    pub bar_bg: (u8, u8, u8, f32),
    pub bar_border: (u8, u8, u8, f32),
    /// chữ và màu nhấn
    pub fg: [u8; 3],
    pub acc: [u8; 3],
    /// 3 chấm màu đại diện (thẻ chọn chủ đề)
    pub dots: [u32; 3],
}

impl Kind {
    /// Mọi chủ đề, theo thứ tự hiện trong cài đặt: các mùa, rồi các ngày lễ trong năm.
    pub const ALL: [Kind; 20] = [
        Kind::Spring,
        Kind::Summer,
        Kind::Autumn,
        Kind::Winter,
        Kind::NewYear,
        Kind::Tet,
        Kind::Valentine,
        Kind::Women,
        Kind::HungKings,
        Kind::Reunify,
        Kind::Children,
        Kind::DoanNgo,
        Kind::Remembrance,
        Kind::National,
        Kind::Moon,
        Kind::VnWomen,
        Kind::Halloween,
        Kind::Teachers,
        Kind::CultureDay,
        Kind::Noel,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Noel => "Giáng sinh",
            Kind::Tet => "Tết Nguyên Đán",
            Kind::Moon => "Trung thu",
            Kind::Spring => "Mùa xuân",
            Kind::Summer => "Mùa hạ",
            Kind::Autumn => "Mùa thu",
            Kind::Winter => "Mùa đông",
            Kind::NewYear => "Năm mới",
            Kind::Valentine => "Valentine",
            Kind::Women => "Quốc tế Phụ nữ 8/3",
            Kind::Reunify => "30/4 – 1/5",
            Kind::Children => "Quốc tế Thiếu nhi 1/6",
            Kind::National => "Quốc khánh 2/9",
            Kind::VnWomen => "Phụ nữ Việt Nam 20/10",
            Kind::Halloween => "Halloween",
            Kind::Teachers => "Nhà giáo Việt Nam 20/11",
            Kind::HungKings => "Giỗ Tổ Hùng Vương",
            Kind::DoanNgo => "Tết Đoan Ngọ",
            Kind::Remembrance => "Ngày Thương binh, Liệt sĩ 27/7",
            Kind::CultureDay => "Ngày Văn hóa Việt Nam 24/11",
        }
    }

    /// Mã lưu trong config.toml.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Noel => "noel",
            Kind::Tet => "tet",
            Kind::Moon => "moon",
            Kind::Spring => "spring",
            Kind::Summer => "summer",
            Kind::Autumn => "autumn",
            Kind::Winter => "winter",
            Kind::NewYear => "newyear",
            Kind::Valentine => "valentine",
            Kind::Women => "women",
            Kind::Reunify => "reunify",
            Kind::Children => "children",
            Kind::National => "national",
            Kind::VnWomen => "vnwomen",
            Kind::Halloween => "halloween",
            Kind::Teachers => "teachers",
            Kind::HungKings => "hung-kings",
            Kind::DoanNgo => "doan-ngo",
            Kind::Remembrance => "remembrance",
            Kind::CultureDay => "culture-day",
        }
    }

    pub fn from_id(id: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.id() == id)
    }

    /// Mô tả ngắn trên thẻ chọn chủ đề.
    pub fn about(self) -> &'static str {
        match self {
            Kind::Noel => "Làng tuyết, xe tuần lộc",
            Kind::Tet => "Sum họp, bánh chưng, mai đào",
            Kind::Moon => "Rước đèn bên sông Hội An",
            Kind::Spring => "Vườn hoa, gió xuân",
            Kind::Summer => "Thuyền thúng giữa vịnh Hạ Long",
            Kind::Autumn => "Cốm mùa thu bên hồ Gươm",
            Kind::Winter => "Phố lạnh và bếp khoai",
            Kind::NewYear => "Pháo hoa bên sông Sài Gòn",
            Kind::Valentine => "Hẹn hò trong vườn hồng",
            Kind::Women => "Trao hoa mimosa",
            Kind::Reunify => "Cờ đỏ, Dinh Độc Lập",
            Kind::Children => "Thả diều trong công viên",
            Kind::National => "Ngày lễ tại Ba Đình",
            Kind::VnWomen => "Áo dài và hoa sen",
            Kind::Halloween => "Phố nhỏ và giỏ kẹo bí ngô",
            Kind::Teachers => "Tặng hoa trong sân trường",
            Kind::HungKings => "Rước kiệu lên Đền Hùng",
            Kind::DoanNgo => "Mâm bánh tro và trái cây ngày hè",
            Kind::Remembrance => "Dâng hoa và thắp nến tưởng niệm",
            Kind::CultureDay => "In tranh Đông Hồ ở sân đình",
        }
    }

    pub fn theme(self) -> Theme {
        let t = |bg: (u8, u8, u8, f32), bd: (u8, u8, u8, f32), fg: [u8; 3], acc: [u8; 3], dots: [u32; 3]| Theme { bar_bg: bg, bar_border: bd, fg, acc, dots };
        match self {
            Kind::Noel => t((17, 55, 54, 0.62), (169, 222, 213, 0.42), [244, 251, 247], [255, 213, 136], [0xf6df9b, 0x75cfad, 0xf28776]),
            Kind::Tet => t((96, 23, 33, 0.68), (255, 210, 118, 0.53), [255, 246, 223], [255, 210, 114], [0xffca64, 0xd9694c, 0xfeae53]),
            Kind::Moon => t((44, 31, 76, 0.67), (244, 174, 102, 0.49), [249, 240, 255], [255, 206, 129], [0xffd087, 0xb886e7, 0xde7169]),
            Kind::Spring => t((39, 68, 58, 0.59), (236, 171, 184, 0.43), [255, 246, 240], [255, 185, 202], [0xf6a5b7, 0x97ba7f, 0xf4d49b]),
            Kind::Summer => t((18, 65, 77, 0.62), (128, 223, 215, 0.42), [238, 252, 250], [156, 239, 229], [0xffc86d, 0x65c8cb, 0xa7eed6]),
            Kind::Autumn => t((78, 44, 32, 0.65), (226, 160, 97, 0.52), [255, 243, 224], [255, 197, 126], [0xef9460, 0xf4ca76, 0xcb6f50]),
            Kind::Winter => t((43, 54, 76, 0.64), (174, 201, 228, 0.46), [246, 249, 255], [189, 219, 247], [0xeaf7ff, 0x8caecb, 0xe8ae76]),
            Kind::NewYear => t((24, 28, 68, 0.68), (191, 173, 247, 0.48), [248, 241, 255], [255, 219, 161], [0xffd695, 0xe99acb, 0x96dcff]),
            Kind::Valentine => t((81, 36, 56, 0.64), (243, 165, 190, 0.47), [255, 240, 247], [255, 173, 198], [0xe987aa, 0xffced9, 0xc86b8e]),
            Kind::Women => t((62, 44, 84, 0.62), (223, 180, 244, 0.45), [251, 243, 255], [238, 199, 249], [0xffd674, 0xdbaaee, 0x9bc690]),
            Kind::VnWomen => t((37, 62, 61, 0.63), (240, 163, 185, 0.49), [251, 245, 239], [255, 178, 202], [0xe88faf, 0xffb9ce, 0x72b791]),
            Kind::Reunify => t((55, 61, 43, 0.65), (232, 166, 117, 0.42), [255, 246, 228], [255, 178, 139], [0xe75c54, 0xffd972, 0x98af73]),
            Kind::National => t((71, 29, 40, 0.67), (244, 199, 111, 0.56), [255, 248, 230], [255, 216, 123], [0xe65957, 0xffd86f, 0xcfa474]),
            Kind::Children => t((33, 61, 88, 0.59), (154, 213, 248, 0.41), [242, 250, 255], [162, 225, 255], [0xf98b84, 0xffd97d, 0xa1dcff]),
            Kind::Halloween => t((47, 28, 59, 0.68), (233, 145, 88, 0.52), [255, 243, 230], [255, 178, 109], [0xee8e48, 0x8b6bc2, 0x683c78]),
            Kind::Teachers => t((51, 64, 49, 0.62), (235, 192, 113, 0.45), [255, 248, 228], [255, 213, 130], [0xf7c66d, 0xf4dcaf, 0xc29569]),
            Kind::HungKings => t((31, 52, 39, 0.76), (214, 173, 94, 0.55), [255, 246, 223], [249, 207, 118], [0xd1a758, 0x627d50, 0xab493f]),
            Kind::DoanNgo => t((67, 39, 65, 0.74), (237, 174, 127, 0.53), [255, 247, 236], [255, 194, 128], [0xa65372, 0x8ea456, 0xf3b575]),
            Kind::Remembrance => t((34, 45, 57, 0.78), (191, 177, 136, 0.48), [248, 245, 237], [231, 201, 137], [0x717e86, 0xe2c489, 0xb8bd9b]),
            Kind::CultureDay => t((37, 47, 73, 0.76), (237, 166, 117, 0.54), [255, 246, 228], [255, 192, 122], [0x59688c, 0xc77550, 0xe6b85e]),
        }
    }

    /// Lời chúc trên đồng hồ hình nền và taskbar.
    pub fn greeting(self) -> &'static str {
        match self {
            Kind::Noel => "Giáng sinh an lành",
            Kind::Tet => "An khang thịnh vượng",
            Kind::Moon => "Trung thu đoàn viên",
            Kind::Spring => "Xuân sang an lành",
            Kind::Summer => "Hạ rực rỡ nắng vàng",
            Kind::Autumn => "Thu về dịu dàng",
            Kind::Winter => "Đông ấm áp yêu thương",
            Kind::NewYear => "Chúc mừng năm mới",
            Kind::Valentine => "Valentine ngọt ngào",
            Kind::Women => "Mừng ngày Quốc tế Phụ nữ",
            Kind::Reunify => "Mừng ngày thống nhất",
            Kind::Children => "Vui Tết Thiếu nhi",
            Kind::National => "Mừng Quốc khánh",
            Kind::VnWomen => "Mừng ngày Phụ nữ Việt Nam",
            Kind::Halloween => "Happy Halloween",
            Kind::Teachers => "Tri ân thầy cô",
            Kind::HungKings => "Nhớ về cội nguồn",
            Kind::DoanNgo => "Tết Đoan Ngọ an lành",
            Kind::Remembrance => "Đời đời ghi nhớ công ơn",
            Kind::CultureDay => "Gìn giữ nét Việt",
        }
    }

    /// Màu chữ đồng hồ/lời chúc trên hình nền khi nền trời sáng (chữ tối cho dễ đọc):
    /// (chữ, màu nhấn). None = nền tối, dùng chữ sáng.
    pub fn clock_on_light(self) -> Option<([u8; 3], [u8; 3])> {
        match self {
            Kind::Tet => Some(([0x47, 0x22, 0x28], [0x7d, 0x22, 0x3c])),
            Kind::Summer => Some(([0x15, 0x30, 0x47], [0x13, 0x49, 0x50])),
            Kind::National => Some(([0x3b, 0x27, 0x3d], [0x6c, 0x2a, 0x41])),
            Kind::Reunify => Some(([0x25, 0x32, 0x26], [0x63, 0x32, 0x22])),
            Kind::Spring => Some(([0x24, 0x34, 0x4d], [0xc2, 0x25, 0x5c])),
            Kind::Autumn => Some(([0x4a, 0x25, 0x11], [0xb3, 0x3c, 0x12])),
            Kind::Winter => Some(([0x2b, 0x3a, 0x4a], [0x2f, 0x5f, 0x96])),
            Kind::Valentine => Some(([0x6b, 0x0f, 0x2e], [0xc2, 0x18, 0x5b])),
            Kind::Women | Kind::VnWomen => Some(([0x3d, 0x1d, 0x5c], [0xc2, 0x18, 0x5b])),
            Kind::Children => Some(([0x16, 0x32, 0x4f], [0xd9, 0x48, 0x0f])),
            Kind::Teachers => Some(([0x5a, 0x35, 0x10], [0xc2, 0x41, 0x0c])),
            Kind::HungKings => Some(([0x24, 0x33, 0x20], [0x68, 0x33, 0x22])),
            Kind::DoanNgo => Some(([0x39, 0x24, 0x36], [0x73, 0x30, 0x50])),
            Kind::Remembrance => Some(([0x23, 0x2d, 0x38], [0x54, 0x40, 0x22])),
            Kind::CultureDay => Some(([0x25, 0x30, 0x47], [0x79, 0x3b, 0x25])),
            _ => None,
        }
    }

    pub fn is_holiday(self) -> bool {
        !matches!(self, Kind::Spring | Kind::Summer | Kind::Autumn | Kind::Winter)
    }
}

/// Khung thiết kế: ngang 1280x720, dọc 720x1280.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub w: f32,
    pub h: f32,
}

impl Layout {
    pub fn for_screen(sw: u32, sh: u32) -> Self {
        if sh > sw { Layout { w: H, h: W } } else { Layout { w: W, h: H } }
    }
    pub fn portrait(&self) -> bool {
        self.h > self.w
    }
}

/// Ảnh minh hoạ riêng do người dùng đặt vào %APPDATA%\season-desktoprt\<chủ đề>\<tên>.png
/// (PNG nền trong suốt): các cảnh dùng ảnh này thay cho hình vẽ bằng code nếu có.
pub fn load_art(theme: &str, name: &str) -> Option<tiny_skia::Pixmap> {
    let path = art_path(theme, name);
    let bytes = std::fs::read(&path).ok()?;
    match tiny_skia::Pixmap::decode_png(&bytes) {
        Ok(p) => {
            crate::log(&format!("dùng ảnh riêng: {}", path.display()));
            Some(p)
        }
        Err(e) => {
            crate::log(&format!("không đọc được ảnh {}: {e} (cần PNG)", path.display()));
            None
        }
    }
}

pub trait Scene: Send {
    fn clock_position(&self, kind: Kind, layout: Layout) -> (f32, f32) {
        crate::widget::position(kind, layout, false)
    }
    fn frame(&mut self, t: f32, dt: f32, cfg: &Config, dl: &mut DrawList);
    /// Cảnh có chuyển động hay không (cảnh tĩnh thì không cần vẽ lại liên tục).
    fn animated(&self) -> bool {
        true
    }
}

struct StillPainting;

impl Scene for StillPainting {
    fn frame(&mut self, _: f32, _: f32, _: &Config, dl: &mut DrawList) {
        dl.layer(Tex::Back);
    }

    fn animated(&self) -> bool {
        false
    }
}

pub struct Built {
    pub layout: Layout,
    pub clock_position: (f32, f32),
    pub scene: Box<dyn Scene>,
    pub back: Canvas,
    pub front: Canvas,
    pub atlas: Atlas,
}

/// Cách đặt khung thiết kế (ngang hoặc dọc) lên màn hình thật: phủ kín, căn giữa.
pub fn fit(sw: u32, sh: u32) -> (f32, [f32; 2], Layout) {
    let l = Layout::for_screen(sw, sh);
    let s = (sw as f32 / l.w).max(sh as f32 / l.h);
    (s, [(sw as f32 - l.w * s) / 2.0, (sh as f32 - l.h * s) / 2.0], l)
}

fn art_path(theme: &str, name: &str) -> std::path::PathBuf {
    crate::config::dir().join("art").join(theme).join(name)
}

pub fn build(kind: Kind, sw: u32, sh: u32) -> Built {
    let (s, off, l) = fit(sw, sh);
    let base = Transform::from_row(s, 0.0, 0.0, s, off[0], off[1]);
    let custom = load_art(kind.id(), "nen.png");
    let painting = if custom.is_none() && !art_path(kind.id(), "nguoi.png").is_file() {
        painting::load(kind, l.portrait())
    } else {
        None
    };
    let painted = painting.is_some();
    // Keep the existing effect builders, but rasterize their unused scenery into a scratch pixel.
    let (bw, bh) = if painted { (1, 1) } else { (sw, sh) };
    let mut back = Canvas::new(bw, bh, base);
    let mut front = Canvas::new(bw, bh, base);
    let mut atlas = Atlas::new(s);
    let scene: Box<dyn Scene> = match kind {
        Kind::Noel => Box::new(noel::Noel::build(&mut back, &mut front, &mut atlas, l, painted)),
        Kind::Tet => Box::new(tet::Tet::build(&mut back, &mut front, &mut atlas, l, painted)),
        Kind::Moon => Box::new(moon::Moon::build(&mut back, &mut front, &mut atlas, l, painted)),
        Kind::HungKings | Kind::DoanNgo | Kind::Remembrance | Kind::CultureDay if painted => Box::new(StillPainting),
        k => Box::new(festive::Festive::build(k, &mut back, &mut front, &mut atlas, l, painted)),
    };
    let default_clock = scene.clock_position(kind, l);
    if let Some(image) = painting {
        let source_scale = (sw as f32 / image.width() as f32)
            .max(sh as f32 / image.height() as f32)
            .max(1.0);
        let width = (sw as f32 / source_scale).round().max(1.0) as u32;
        let height = (sh as f32 / source_scale).round().max(1.0) as u32;
        if (width, height) == (image.width(), image.height()) {
            back.pm = image;
        } else {
            back = Canvas::new(width, height, base);
            painting::draw(&mut back, &image);
        }
        front.pm.fill(tiny_skia::Color::TRANSPARENT);
        let clock_position = painting::clock_clearance(
            &back.pm, (sw, sh), s, off, default_clock,
            kind.clock_on_light().is_some(), kind == Kind::Noel,
        );
        return Built { layout: l, clock_position, scene, back, front, atlas };
    }
    // Ảnh nền riêng của người dùng (art\<chủ đề>\nen.png): phủ kín màn hình thay cho bầu trời,
    // đồi núi vẽ bằng code; hoạ tiết, nhân vật, hiệu ứng vẫn vẽ bên trên.
    if let Some(img) = custom {
        let s = (sw as f32 / img.width().max(1) as f32).max(sh as f32 / img.height().max(1) as f32);
        let (ox, oy) = ((sw as f32 - img.width() as f32 * s) / 2.0, (sh as f32 - img.height() as f32 * s) / 2.0);
        back.pm.fill(tiny_skia::Color::BLACK);
        back.pm.draw_pixmap(0, 0, img.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, Transform::from_translate(ox, oy).pre_scale(s, s), None);
    }
    depth::near(&mut front, kind, l);
    foreground::draw(&mut front, kind, l);
    Built { layout: l, clock_position: default_clock, scene, back, front, atlas }
}
