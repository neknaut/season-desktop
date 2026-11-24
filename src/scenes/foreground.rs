use super::{Kind, Layout};
use crate::paint::{ca, solid, Canvas};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint};

fn bytes(kind: Kind) -> &'static [u8] {
    match kind {
        Kind::Spring => include_bytes!("../../assets/foregrounds/spring.png"),
        Kind::Summer => include_bytes!("../../assets/foregrounds/summer.png"),
        Kind::Autumn => include_bytes!("../../assets/foregrounds/autumn.png"),
        Kind::Winter => include_bytes!("../../assets/foregrounds/winter.png"),
        Kind::Tet => &[],
        Kind::Moon => include_bytes!("../../assets/foregrounds/moon.png"),
        Kind::Noel => include_bytes!("../../assets/foregrounds/noel.png"),
        Kind::NewYear => include_bytes!("../../assets/foregrounds/newyear.png"),
        Kind::Valentine => include_bytes!("../../assets/foregrounds/valentine.png"),
        Kind::Women => include_bytes!("../../assets/foregrounds/women.png"),
        Kind::VnWomen => include_bytes!("../../assets/foregrounds/vnwomen.png"),
        Kind::Reunify => include_bytes!("../../assets/foregrounds/reunify.png"),
        Kind::National => include_bytes!("../../assets/foregrounds/national.png"),
        Kind::Children => include_bytes!("../../assets/foregrounds/children.png"),
        Kind::Halloween => include_bytes!("../../assets/foregrounds/halloween.png"),
        Kind::Teachers => include_bytes!("../../assets/foregrounds/teachers.png"),
    }
}

fn sprite(kind: Kind) -> Option<Pixmap> {
    let mut image = Pixmap::decode_png(bytes(kind)).ok()?;
    let gain: [f32; 3] = match kind {
        Kind::Spring | Kind::Children => [1.03, 1.02, 0.95],
        Kind::Summer => [0.96, 1.03, 1.05],
        Kind::Autumn | Kind::Tet | Kind::Teachers => [1.08, 1.01, 0.91],
        Kind::Winter | Kind::Noel => [0.92, 1.00, 1.09],
        Kind::Moon | Kind::NewYear | Kind::Halloween => [0.94, 0.90, 1.09],
        Kind::Valentine | Kind::Women | Kind::VnWomen => [1.06, 0.96, 1.01],
        Kind::Reunify | Kind::National => [1.05, 1.00, 0.95],
    };
    for pixel in image.data_mut().chunks_exact_mut(4) {
        let alpha = pixel[3] as f32;
        let shade = 1.0 - pixel[..3].iter().copied().max().unwrap_or(0) as f32
            / alpha.max(1.0) * 0.55;
        for (channel, gain) in pixel[..3].iter_mut().zip(gain) {
            let adjusted = *channel as f32 * (1.0 + (gain - 1.0) * shade);
            *channel = adjusted.round().clamp(0.0, alpha) as u8;
        }
    }
    Some(image)
}

fn ground_detail(cv: &mut Canvas, kind: Kind, l: Layout, floor: f32) {
    // Small, subdued details stay along the bottom and leave the desktop icon area open.
    let accents = match kind {
        Kind::Spring | Kind::Children => [(0.76, 0.97), (0.91, 0.92)],
        Kind::Winter | Kind::Noel => [(0.14, 0.97), (0.83, 0.96)],
        Kind::Moon | Kind::NewYear => [(0.82, 0.96), (0.95, 0.98)],
        Kind::Reunify | Kind::National => [(0.66, 0.97), (0.89, 0.98)],
        _ => return,
    };
    let (tone, light) = match kind {
        Kind::Winter | Kind::Noel => ([74, 101, 147], [253, 251, 247]),
        Kind::Spring | Kind::Children => ([50, 82, 48], [225, 223, 160]),
        Kind::Reunify => ([94, 53, 40], [235, 167, 120]),
        _ => ([50, 49, 80], [185, 163, 144]),
    };
    let [r, g, b] = tone;
    let [lr, lg, lb] = light;
    for (x, y) in accents {
        let (x, y) = (l.w * x, (l.h * y).max(floor + 6.0));
        for i in 0..5 {
            let i = i as f32;
            let offset = i - 2.0;
            let px = x + offset * 13.0;
            let py = y + offset.abs() * 2.4;
            let radius = 5.0 + (2.0 - offset.abs()) * 2.8;
            cv.ellipse(px, py + 2.3, radius * 1.36, radius * 0.28, 0.04,
                solid(ca(r, g, b, 0.24)));
            cv.ellipse(px, py, radius, radius * 0.29, -0.12,
                solid(ca(lr, lg, lb, 0.28)));
        }
    }
}

pub(super) fn draw(cv: &mut Canvas, kind: Kind, l: Layout) {
    let (wide_x, tall_x, wide_floor, tall_floor, height) = match kind {
        Kind::Tet => return,
        Kind::Spring => (0.56, 0.54, 0.94, 0.92, 0.23),
        Kind::Summer => (0.48, 0.43, 0.81, 0.79, 0.17),
        Kind::Autumn => (0.55, 0.56, 0.85, 0.80, 0.17),
        Kind::Winter => (0.49, 0.49, 0.93, 0.93, 0.23),
        Kind::Noel => (0.52, 0.47, 0.94, 0.94, 0.22),
        Kind::Moon => (0.68, 0.55, 0.94, 0.94, 0.23),
        Kind::NewYear => (0.32, 0.35, 0.95, 0.95, 0.23),
        Kind::Valentine => (0.42, 0.39, 0.94, 0.93, 0.23),
        Kind::Women | Kind::VnWomen => (0.69, 0.72, 0.94, 0.94, 0.23),
        Kind::Reunify | Kind::National => (0.30, 0.33, 0.94, 0.94, 0.22),
        Kind::Children => (0.54, 0.57, 0.94, 0.93, 0.23),
        Kind::Halloween => (0.48, 0.52, 0.94, 0.94, 0.24),
        Kind::Teachers => (0.66, 0.56, 0.94, 0.93, 0.25),
    };
    if super::art_path(kind.id(), "nguoi.png").is_file()
        && !matches!(kind, Kind::Noel | Kind::Moon)
    {
        return;
    }
    let Some(image) = sprite(kind) else { return };
    let x = l.w * if l.portrait() { tall_x } else { wide_x };
    let y = l.h * if l.portrait() { tall_floor } else { wide_floor };
    let scale = (l.h * height / image.height() as f32)
        .min(l.w * if l.portrait() { 0.62 } else { 0.38 } / image.width() as f32);
    let width = scale * image.width() as f32;
    let height = scale * image.height() as f32;
    if matches!(kind, Kind::Moon | Kind::NewYear | Kind::National) {
        let ground = match kind { Kind::Moon => 0x0b0d24, Kind::National => 0x161c33, _ => 0x0b1026 };
        cv.hill(
            |xx| y - 13.0 + ((xx - x) / l.w * 4.0).sin() * 9.0,
            solid(crate::paint::c(ground)),
        );
    }
    ground_detail(cv, kind, l, y);
    cv.ellipse(x, y - height * 0.045, width * 0.34, height * 0.032, 0.0,
        solid(ca(20, 12, 31, 0.22)));
    let transform = cv.base.pre_translate(x - width * 0.5, y - height).pre_scale(scale, scale);
    cv.pm.draw_pixmap(0, 0, image.as_ref(), &PixmapPaint {
        quality: FilterQuality::Bicubic,
        ..Default::default()
    }, transform, None);
}
