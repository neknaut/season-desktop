use super::Kind;
use crate::paint::Canvas;
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

pub(super) fn load(kind: Kind, portrait: bool) -> Option<Pixmap> {
    let bytes: &[u8] = match (kind, portrait) {
        (Kind::Tet, true) => include_bytes!("../../assets/paintings/tet-portrait.png"),
        (Kind::Moon, true) => include_bytes!("../../assets/paintings/moon-portrait.png"),
        (Kind::Winter, true) => include_bytes!("../../assets/paintings/winter-portrait.png"),
        (Kind::Teachers, true) => include_bytes!("../../assets/paintings/teachers-portrait.png"),
        (Kind::HungKings, true) => include_bytes!("../../assets/paintings/hung-kings-portrait.png"),
        (Kind::DoanNgo, true) => include_bytes!("../../assets/paintings/doan-ngo-portrait.png"),
        (Kind::Remembrance, true) => include_bytes!("../../assets/paintings/remembrance-portrait.png"),
        (Kind::CultureDay, true) => include_bytes!("../../assets/paintings/culture-day-portrait.png"),
        (Kind::Noel, true) => include_bytes!("../../assets/paintings/noel-portrait.png"),
        (Kind::Spring, true) => include_bytes!("../../assets/paintings/spring-portrait.png"),
        (Kind::Summer, true) => include_bytes!("../../assets/paintings/summer-portrait.png"),
        (Kind::Autumn, true) => include_bytes!("../../assets/paintings/autumn-portrait.png"),
        (Kind::NewYear, true) => include_bytes!("../../assets/paintings/newyear-portrait.png"),
        (Kind::Valentine, true) => include_bytes!("../../assets/paintings/valentine-portrait.png"),
        (Kind::Women, true) => include_bytes!("../../assets/paintings/women-portrait.png"),
        (Kind::Reunify, true) => include_bytes!("../../assets/paintings/reunify-portrait.png"),
        (Kind::Children, true) => include_bytes!("../../assets/paintings/children-portrait.png"),
        (Kind::National, true) => include_bytes!("../../assets/paintings/national-portrait.png"),
        (Kind::VnWomen, true) => include_bytes!("../../assets/paintings/vnwomen-portrait.png"),
        (Kind::Halloween, true) => include_bytes!("../../assets/paintings/halloween-portrait.png"),
        (Kind::Noel, _) => include_bytes!("../../assets/paintings/noel.png"),
        (Kind::Tet, _) => include_bytes!("../../assets/paintings/tet.png"),
        (Kind::Moon, _) => include_bytes!("../../assets/paintings/moon.png"),
        (Kind::Spring, _) => include_bytes!("../../assets/paintings/spring.png"),
        (Kind::Summer, _) => include_bytes!("../../assets/paintings/summer.png"),
        (Kind::Autumn, _) => include_bytes!("../../assets/paintings/autumn.png"),
        (Kind::Winter, _) => include_bytes!("../../assets/paintings/winter.png"),
        (Kind::NewYear, _) => include_bytes!("../../assets/paintings/newyear.png"),
        (Kind::Valentine, _) => include_bytes!("../../assets/paintings/valentine.png"),
        (Kind::Women, _) => include_bytes!("../../assets/paintings/women.png"),
        (Kind::Reunify, _) => include_bytes!("../../assets/paintings/reunify.png"),
        (Kind::Children, _) => include_bytes!("../../assets/paintings/children.png"),
        (Kind::National, _) => include_bytes!("../../assets/paintings/national.png"),
        (Kind::VnWomen, _) => include_bytes!("../../assets/paintings/vnwomen.png"),
        (Kind::Halloween, _) => include_bytes!("../../assets/paintings/halloween.png"),
        (Kind::Teachers, _) => include_bytes!("../../assets/paintings/teachers.png"),
        (Kind::HungKings, _) => include_bytes!("../../assets/paintings/hung-kings.png"),
        (Kind::DoanNgo, _) => include_bytes!("../../assets/paintings/doan-ngo.png"),
        (Kind::Remembrance, _) => include_bytes!("../../assets/paintings/remembrance.png"),
        (Kind::CultureDay, _) => include_bytes!("../../assets/paintings/culture-day.png"),
    };
    Pixmap::decode_png(bytes).ok()
}

pub(super) fn draw(cv: &mut Canvas, image: &Pixmap) {
    let (w, h) = (cv.pm.width() as f32, cv.pm.height() as f32);
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let scale = (w / iw).max(h / ih);
    let origin = ((w - iw * scale) * 0.5, (h - ih * scale) * 0.5);
    cv.pm.draw_pixmap(
        0, 0, image.as_ref(),
        &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() },
        Transform::from_translate(origin.0, origin.1).pre_scale(scale, scale),
        None,
    );
}


pub(crate) fn thumbnail(kind: Kind, portrait: bool) -> Option<Pixmap> {
    if let Some(image) = super::load_art(kind.id(), "nen.png") {
        let (w, h) = if portrait { (180u32, 320u32) } else { (320, 180) };
        let mut preview = Pixmap::new(w, h)?;
        let (w, h) = (w as f32, h as f32);
        let scale = (w / image.width() as f32).max(h / image.height() as f32);
        let x = (w - image.width() as f32 * scale) * 0.5;
        let y = (h - image.height() as f32 * scale) * 0.5;
        preview.draw_pixmap(0, 0, image.as_ref(),
            &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() },
            Transform::from_translate(x, y).pre_scale(scale, scale), None);
        return Some(preview);
    }

    let bytes: &[u8] = match (kind, portrait) {
        (Kind::Spring, true) => include_bytes!("../../assets/previews/spring-portrait.png"),
        (Kind::Spring, false) => include_bytes!("../../assets/previews/spring.png"),
        (Kind::Summer, true) => include_bytes!("../../assets/previews/summer-portrait.png"),
        (Kind::Summer, false) => include_bytes!("../../assets/previews/summer.png"),
        (Kind::Autumn, true) => include_bytes!("../../assets/previews/autumn-portrait.png"),
        (Kind::Autumn, false) => include_bytes!("../../assets/previews/autumn.png"),
        (Kind::Winter, true) => include_bytes!("../../assets/previews/winter-portrait.png"),
        (Kind::Winter, false) => include_bytes!("../../assets/previews/winter.png"),
        (Kind::NewYear, true) => include_bytes!("../../assets/previews/newyear-portrait.png"),
        (Kind::NewYear, false) => include_bytes!("../../assets/previews/newyear.png"),
        (Kind::Tet, true) => include_bytes!("../../assets/previews/tet-portrait.png"),
        (Kind::Tet, false) => include_bytes!("../../assets/previews/tet.png"),
        (Kind::Valentine, true) => include_bytes!("../../assets/previews/valentine-portrait.png"),
        (Kind::Valentine, false) => include_bytes!("../../assets/previews/valentine.png"),
        (Kind::Women, true) => include_bytes!("../../assets/previews/women-portrait.png"),
        (Kind::Women, false) => include_bytes!("../../assets/previews/women.png"),
        (Kind::Reunify, true) => include_bytes!("../../assets/previews/reunify-portrait.png"),
        (Kind::Reunify, false) => include_bytes!("../../assets/previews/reunify.png"),
        (Kind::Children, true) => include_bytes!("../../assets/previews/children-portrait.png"),
        (Kind::Children, false) => include_bytes!("../../assets/previews/children.png"),
        (Kind::National, true) => include_bytes!("../../assets/previews/national-portrait.png"),
        (Kind::National, false) => include_bytes!("../../assets/previews/national.png"),
        (Kind::Moon, true) => include_bytes!("../../assets/previews/moon-portrait.png"),
        (Kind::Moon, false) => include_bytes!("../../assets/previews/moon.png"),
        (Kind::VnWomen, true) => include_bytes!("../../assets/previews/vnwomen-portrait.png"),
        (Kind::VnWomen, false) => include_bytes!("../../assets/previews/vnwomen.png"),
        (Kind::Halloween, true) => include_bytes!("../../assets/previews/halloween-portrait.png"),
        (Kind::Halloween, false) => include_bytes!("../../assets/previews/halloween.png"),
        (Kind::Teachers, true) => include_bytes!("../../assets/previews/teachers-portrait.png"),
        (Kind::HungKings, true) => include_bytes!("../../assets/previews/hung-kings-portrait.png"),
        (Kind::DoanNgo, true) => include_bytes!("../../assets/previews/doan-ngo-portrait.png"),
        (Kind::Remembrance, true) => include_bytes!("../../assets/previews/remembrance-portrait.png"),
        (Kind::CultureDay, true) => include_bytes!("../../assets/previews/culture-day-portrait.png"),
        (Kind::Teachers, false) => include_bytes!("../../assets/previews/teachers.png"),
        (Kind::HungKings, false) => include_bytes!("../../assets/previews/hung-kings.png"),
        (Kind::DoanNgo, false) => include_bytes!("../../assets/previews/doan-ngo.png"),
        (Kind::Remembrance, false) => include_bytes!("../../assets/previews/remembrance.png"),
        (Kind::CultureDay, false) => include_bytes!("../../assets/previews/culture-day.png"),
        (Kind::Noel, true) => include_bytes!("../../assets/previews/noel-portrait.png"),
        (Kind::Noel, false) => include_bytes!("../../assets/previews/noel.png"),
    };
    Pixmap::decode_png(bytes).ok()
}

pub(super) fn clock_clearance(
    image: &Pixmap,
    screen: (u32, u32),
    scale: f32,
    offset: [f32; 2],
    preferred: (f32, f32),
    dark_ink: bool,
    keep_left: bool,
) -> (f32, f32) {
    let (sw, sh) = (screen.0 as f32, screen.1 as f32);
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let (visible_w, visible_h) = (sw / scale, sh / scale);
    let left = -offset[0] / scale;
    let top = -offset[1] / scale;
    let rect_w = 420.0f32.min(visible_w * 0.76);
    let rect_h = 195.0f32.min(visible_h * 0.37);
    let margin = (visible_w * 0.055).clamp(27.0, 64.0);
    let x_min = left + margin;
    let x_max = (left + visible_w - rect_w - margin).max(x_min);
    let y_min = top + (visible_h * 0.057).clamp(30.0, 72.0);
    let y_max = y_min + (visible_h * 0.065).min(84.0);
    let origin = (preferred.0.clamp(x_min, x_max.max(x_min)), preferred.1.clamp(y_min, y_max));
    let positions = [
        origin,
        (x_min, y_max),
        ((x_min + x_max) * 0.5, y_min),
        (x_max, y_min),
        (x_max, y_max),
    ];
    let luminance = |x: f32, y: f32| {
        let x = ((x * scale + offset[0]) * iw / sw).clamp(0.0, iw - 1.0) as u32;
        let y = ((y * scale + offset[1]) * ih / sh).clamp(0.0, ih - 1.0) as u32;
        let p = image.pixel(x, y).unwrap();
        (f32::from(p.red()) * 0.2126 + f32::from(p.green()) * 0.7152
            + f32::from(p.blue()) * 0.0722) / 255.0
    };
    let score = |(x, y): (f32, f32)| {
        let mut previous = [0.0f32; 16];
        let mut variation = 0.0;
        let mut contrast = 0.0;
        for row in 0..11 {
            let mut last = 0.0f32;
            for (column, above) in previous.iter_mut().enumerate() {
                let u = (column as f32 + 0.5) / 16.0;
                let v = (row as f32 + 0.5) / 11.0;
                let reading = luminance(x + rect_w * u, y + rect_h * v);
                if column > 0 {
                    variation += (reading - last).abs();
                }
                if row > 0 {
                    variation += (reading - *above).abs();
                }
                contrast += if dark_ink {
                    (0.56 - reading).max(0.0)
                } else {
                    (reading - 0.47).max(0.0)
                };
                last = reading;
                *above = reading;
            }
        }
        let travel = ((x - origin.0).abs() + (y - origin.1).abs()) / visible_w;
        (variation * 1.5 + contrast) / 176.0 + travel * 0.065
    };
    let mut best = origin;
    let mut best_score = score(origin);
    let count = if keep_left { 2 } else { positions.len() };
    for &position in &positions[1..count] {
        let current = score(position);
        if current < best_score {
            best_score = current;
            best = position;
        }
    }
    best
}
