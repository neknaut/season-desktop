use crate::paint::{c, ca, lin, solid, Atlas, Canvas};
use tiny_skia::PathBuilder;

#[derive(Clone, Copy)]
pub(super) enum Shape {
    Petal,
    Apricot,
    Leaf,
}

pub(super) fn stamp(atlas: &mut Atlas, size: f32, base: u32, shape: Shape) -> [f32; 4] {
    atlas.add(size, size, size * 0.5, size * 0.5, |cv| {
        let original = cv.base;
        cv.base = original.pre_scale(size / 22.0, size / 22.0);
        paint(cv, base, shape);
        cv.base = original;
    })
}

fn paint(cv: &mut Canvas, base: u32, shape: Shape) {
    let mut path = PathBuilder::new();
    match shape {
        Shape::Petal => {
            path.move_to(-7.5, 2.2);
            path.cubic_to(-4.4, -3.7, 1.6, -7.1, 6.2, -6.4);
            path.quad_to(8.5, -6.0, 7.0, -2.6);
            path.quad_to(10.5, -2.3, 7.4, 0.6);
            path.cubic_to(3.2, 4.5, -4.2, 5.3, -7.5, 2.2);
        }
        Shape::Apricot => {
            path.move_to(-8.3, 3.0);
            path.quad_to(-6.2, -1.6, -3.2, -4.7);
            path.cubic_to(1.6, -9.1, 8.0, -7.1, 8.5, -2.7);
            path.cubic_to(9.2, 2.6, 3.4, 6.1, -1.1, 5.0);
            path.quad_to(-5.4, 4.1, -8.3, 3.0);
        }
        Shape::Leaf => {
            path.move_to(-1.0, 7.0);
            path.quad_to(-6.0, 6.1, -8.7, 1.1);
            path.quad_to(-5.0, 1.4, -5.6, -4.4);
            path.line_to(-2.3, -2.6);
            path.quad_to(-1.4, -7.3, 0.4, -9.2);
            path.quad_to(1.2, -6.0, 4.2, -4.3);
            path.line_to(4.0, -1.8);
            path.line_to(8.6, -2.9);
            path.quad_to(8.1, 2.8, 3.2, 5.7);
            path.quad_to(1.9, 6.6, -1.0, 7.0);
        }
    }
    path.close();
    let Some(path) = path.finish() else { return };
    let color = |amount: f32, target: [u8; 3]| {
        let channels = [(base >> 16) as u8, (base >> 8) as u8, base as u8];
        let [r, g, b] = std::array::from_fn(|i| {
            (channels[i] as f32 * (1.0 - amount) + target[i] as f32 * amount).round() as u8
        });
        ca(r, g, b, 1.0)
    };
    cv.fill(&path, lin(-4.0, -6.0, 4.0, 6.0, &[
        (0.0, color(0.34, [255, 226, 174])),
        (0.55, c(base)),
        (1.0, color(0.45, [79, 45, 53])),
    ]));
    cv.stroke(&path, solid(ca(72, 40, 37, 0.74)), 0.72);
    match shape {
        Shape::Leaf => {
            let stem = solid(ca(102, 54, 39, 0.76));
            cv.line(-1.8, 9.8, 0.3, -5.8, stem.clone(), 0.78);
            cv.line(-0.7, 3.6, -5.1, 0.2, stem.clone(), 0.65);
            cv.line(-0.5, 2.0, 4.9, 0.5, stem, 0.65);
        }
        Shape::Petal | Shape::Apricot => {
            cv.line(-4.7, 1.8, 1.2, -2.0, solid(ca(255, 232, 194, 0.42)), 0.68);
        }
    }
}
