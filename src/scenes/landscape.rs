use super::{Kind, Layout};
use crate::paint::{c, ca, lin, solid, Canvas};
use tiny_skia::PathBuilder;

fn silhouette(cv: &mut Canvas, points: &[(f32, f32)], ink: u32) {
    let mut path = PathBuilder::new();
    path.move_to(points[0].0, points[0].1);
    for &(x, y) in &points[1..] {
        path.line_to(x, y);
    }
    path.close();
    if let Some(path) = path.finish() {
        cv.fill(&path, solid(c(ink)));
    }
}

fn row_houses(cv: &mut Canvas, l: Layout, y: f32, walls: [u32; 3], roof: u32) {
    let spacing = if l.portrait() { 75.0 } else { 92.0 };
    for i in 0..=(l.w / spacing) as usize {
        let x = i as f32 * spacing - 12.0;
        let w = spacing - 4.0;
        let h = spacing * (0.52 + (i * 7 % 5) as f32 * 0.14);
        let top = y - h;
        let wall = c(walls[i % walls.len()]);
        cv.rect(x, top, w, h, lin(x + w * 0.30, top, x + w * 0.85, y, &[
            (0.0, wall),
            (0.72, wall),
            (1.0, c(roof)),
        ]));
        silhouette(cv, &[(x - 5.0, top), (x + w * 0.26, top - 18.0),
            (x + w * 0.78, top - 18.0), (x + w + 4.0, top)], roof);
        cv.line(x + w * 0.22, top - 13.0, x + w * 0.82, top - 13.0,
            solid(ca(250, 201, 160, 0.25)), 1.9);
        cv.line(x - 5.0, top + 1.0, x + w + 4.0, top + 1.0,
            solid(ca(24, 18, 29, 0.48)), 3.4);
        for j in 0..3 {
            let window = x + w * (0.15 + j as f32 * 0.27);
            let glass = if (i * 3 + j) % 4 == 0 { 0x766574 } else { 0xf1c584 };
            cv.round_rect(window, top + 8.0, w * 0.14, 18.0, 2.2, solid(c(roof)));
            cv.rect(window + 2.0, top + 10.0, w * 0.14 - 4.0, 13.0, solid(c(glass)));
            cv.line(window + w * 0.07, top + 10.0, window + w * 0.07, top + 23.0,
                solid(c(roof)), 1.5);
            cv.line(window - 1.0, top + 26.0, window + w * 0.14 + 1.0, top + 26.0,
                solid(ca(25, 22, 34, 0.37)), 2.0);
        }
        cv.round_rect(x + w * 0.38, y - 24.0, w * 0.24, 25.0, 2.3, solid(c(roof)));
        if i % 3 != 1 {
            for offset in [0.29, 0.71] {
                let lx = x + w * offset;
                cv.glow(lx, y - 23.0, 25.0, (244, 147, 81), 0.26);
                cv.ellipse(lx, y - 23.0, 3.4, 5.0, 0.0, solid(c(0xeeb374)));
            }
        }
    }
}

fn firs(cv: &mut Canvas, l: Layout, y: f32, inks: [u32; 2]) {
    for (layer, ink) in inks.into_iter().enumerate() {
        let step = 38.0 + layer as f32 * 11.0;
        for i in 0..=(l.w / step) as usize {
            let x = step * (i as f32 + 0.44 * layer as f32);
            let height = 32.0 + (i * 11 % 17) as f32 * 3.5;
            for branch in 0..3 {
                let tip = y - height + height * 0.20 * branch as f32;
                let foot = tip + height * 0.55;
                let radius = height * (0.21 + branch as f32 * 0.055);
                silhouette(cv, &[(x, tip), (x - radius, foot), (x + radius, foot)], ink);
            }
        }
    }
}

pub(super) fn water(cv: &mut Canvas, l: Layout, y: f32, colors: [u32; 2], reflection: [u8; 3], light: f32) {
    let depth = l.h - y;
    cv.rect(0.0, y, l.w, depth, lin(0.0, y, 0.0, y + depth,
        &[(0.0, c(colors[0])), (1.0, c(colors[1]))]));
    let [red, green, blue] = reflection;
    for i in 0..100 {
        let t = (i * 43 % 101) as f32 / 101.0;
        let spread = l.w * (0.20 + t * 0.79);
        let x = l.w * light + ((i * 73 % 103) as f32 / 103.0 - 0.5) * spread;
        let yy = y + t * depth;
        let width = (3.0 + (i * 29 % 19) as f32) * (0.40 + t);
        cv.ellipse(x, yy, width, 0.5 + t * 0.8, 0.0,
            solid(ca(red, green, blue, 0.26 - t * 0.16)));
    }
}

fn terraces(cv: &mut Canvas, l: Layout, y: f32) {
    let colors = [0x779957, 0x91aa61, 0xa7b873, 0x77995a, 0x86a768];
    for i in 0..9 {
        let t = i as f32 / 8.0;
        let height = y + t * t * l.h * 0.16;
        let curve = |x: f32| height + (x / l.w * 6.0 + t * 1.5).sin() * (9.0 + t * 18.0);
        cv.hill(curve, solid(c(colors[i % colors.len()])));
        let mut path = PathBuilder::new();
        path.move_to(0.0, curve(0.0));
        for x in (8..=l.w as u32).step_by(8) {
            path.line_to(x as f32, curve(x as f32));
        }
        if let Some(path) = path.finish() {
            cv.stroke(&path, solid(ca(245, 224, 157, 0.50)), 1.0 + t * 1.5);
        }
    }
}

fn conservatory(cv: &mut Canvas, l: Layout, x: f32, y: f32, frames: u32, glass: u32) {
    let unit = l.w.min(850.0) * 0.045;
    let x = l.w * x;
    let left = x - unit * 5.0;
    let top = y - unit * 2.2;
    cv.rect(left, top, unit * 10.0, y - top, solid(c(glass)));
    silhouette(cv, &[(left, top), (x, top - unit * 1.3), (left + unit * 10.0, top)], glass);
    cv.line(left, top, x, top - unit * 1.3, solid(c(frames)), 3.0);
    cv.line(x, top - unit * 1.3, left + unit * 10.0, top, solid(c(frames)), 3.0);
    cv.line(left, top, left + unit * 10.0, top, solid(c(frames)), 3.0);
    for i in 0..=10 {
        let xx = left + unit * i as f32;
        cv.line(xx, top - unit * 1.3 * (1.0 - (xx - x).abs() / (unit * 5.0)),
            xx, y, solid(c(frames)), 2.0);
    }
}

fn civic_hall(cv: &mut Canvas, l: Layout, y: f32, reunification: bool) {
    let w = l.w * if l.portrait() { 0.62 } else { 0.51 };
    let x = (l.w - w) * 0.5;
    let h = w * if reunification { 0.20 } else { 0.26 };
    let (facade, trim) = if reunification { (0xe2d1aa, 0x9b8663) } else { (0xc2c2aa, 0x7e8279) };
    cv.rect(x, y - h, w, h, lin(x, y - h, x, y, &[
        (0.0, c(facade)),
        (1.0, c(trim)),
    ]));
    cv.rect(x - w * 0.03, y - h, w * 1.06, h * 0.13, solid(c(trim)));
    if reunification {
        // Palace: an elevated central pavilion and repeated facade screens.
        cv.rect(x + w * 0.38, y - h * 1.24, w * 0.24, h * 0.24, solid(c(facade)));
        cv.rect(x + w * 0.34, y - h * 1.25, w * 0.32, h * 0.065, solid(c(trim)));
        for i in 0..20 {
            let xx = x + w * (0.022 + i as f32 * 0.048);
            cv.rect(xx, y - h * 0.78, w * 0.023, h * 0.47, solid(c(0x74694f)));
            cv.line(xx, y - h * 0.52, xx + w * 0.023, y - h * 0.52, solid(c(facade)), 1.4);
        }
        cv.rect(x + w * 0.40, y - h * 0.24, w * 0.20, h * 0.20, solid(c(0x645943)));
    } else {
        // Mausoleum: a deep central recess framed by heavy stone columns.
        cv.rect(x + w * 0.08, y - h * 0.77, w * 0.84, h * 0.68, solid(c(0x515958)));
        cv.rect(x - w * 0.035, y - h * 0.98, w * 1.07, h * 0.18, solid(c(facade)));
        for i in 0..8 {
            let xx = x + w * (0.06 + i as f32 * 0.12);
            cv.rect(xx, y - h * 0.80, w * 0.065, h * 0.69, solid(c(facade)));
            cv.rect(xx + w * 0.049, y - h * 0.80, w * 0.016, h * 0.69, solid(c(trim)));
        }
    }
    for i in 0..3 {
        let inset = w * 0.035 * i as f32;
        cv.rect(x - inset, y - h * 0.13 + i as f32 * h * 0.07,
            w + 2.0 * inset, h * 0.08, solid(c(trim)));
    }
}

pub(super) fn paint(kind: Kind, cv: &mut Canvas, l: Layout) {
    super::depth::distance(cv, kind, l);
    let y = l.h * if l.portrait() { 0.75 } else { 0.73 };
    match kind {
        Kind::Spring => terraces(cv, l, y - l.h * 0.06),
        Kind::Summer => {} // Limestone islands are painted before the water, in Festive.
        Kind::Autumn => water(cv, l, y - l.h * 0.02, [0x9e9d7c, 0x595d55], [255, 207, 135], 0.58),
        Kind::Winter => {
            row_houses(cv, l, y, [0x8d8d88, 0xa39684, 0x939890], 0x555c61);
            water(cv, l, y, [0x7f8382, 0x4b535e], [214, 205, 180], 0.48);
        }
        Kind::NewYear => {} // The waterfront is painted after the skyline in Festive.
        Kind::Valentine => {
            water(cv, l, y, [0xb06f83, 0x683952], [255, 194, 186], 0.62);
            conservatory(cv, l, 0.62, y, 0x8c4e67, 0xeab0b6);
        }
        Kind::Women => conservatory(cv, l, 0.65, y, 0x847366, 0xf4cebf),
        Kind::VnWomen => {
            row_houses(cv, l, y, [0xe0a259, 0xe9bd7e, 0xd7a57b], 0x795759);
            water(cv, l, y, [0xab8070, 0x674853], [255, 196, 129], 0.43);
        }
        Kind::Reunify => civic_hall(cv, l, y, true),
        Kind::National => civic_hall(cv, l, y, false),
        Kind::Children => terraces(cv, l, y + l.h * 0.025),
        Kind::Halloween => {
            firs(cv, l, y, [0x403353, 0x292138]);
            row_houses(cv, l, y + l.h * 0.033, [0x3b304a, 0x49334c, 0x352b45], 0x242137);
        }
        Kind::Teachers => row_houses(cv, l, y, [0xd3a275, 0xe1ba86, 0xd8ac7d], 0x86635a),
        Kind::Noel => firs(cv, l, l.h * 0.62, [0x355483, 0x233e64]),
        Kind::Tet => row_houses(cv, l, l.h * 0.82, [0x8b4537, 0x995442, 0x81413b], 0x522932),
        Kind::Moon => {
            let bank = l.h * if l.portrait() { 0.68 } else { 0.72 };
            row_houses(cv, l, bank, [0x9c6649, 0xac7949, 0x8b5747], 0x362f4c);
            water(cv, l, bank, [0x373254, 0x1a2045], [255, 189, 109], 0.54);
        }
    }
}
