//! Ảnh xem trước cửa sổ khi rê chuột vào app ở đảo giữa (như taskbar gốc): mỗi cửa sổ một thẻ
//! gồm icon + tiêu đề + ảnh thu nhỏ trực tiếp của Windows (DWM), bấm để chuyển tới, có nút đóng.

use tiny_skia::*;
use windows::Win32::Foundation::{HWND, RECT};

use super::menu::Theme;
use super::shell;
use crate::paint::{ca, round_rect_path, solid, Canvas};
use crate::widget;

/// Kích thước theo tỉ lệ 100%.
const PAD: f32 = 8.0;
const CARD_W: f32 = 216.0;
const TITLE_H: f32 = 30.0;
const THUMB_H: f32 = 124.0;
const GAP: f32 = 4.0;

pub enum Cmd {
    Activate(isize),
    Close(isize),
}

struct Card {
    win: isize,
    title: String,
    thumb: Option<isize>,
}

pub struct Preview {
    pub hwnd: HWND,
    pub open: bool,
    /// Ô app đang được xem trước
    pub tile: isize,
    cards: Vec<Card>,
    icon: Option<Pixmap>,
    theme: Option<Theme>,
    hover: Option<usize>,
    hover_close: bool,
    k: f32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    /// Chuột đang ở trong bảng xem trước
    pub inside: bool,
    /// Hiệu ứng mở (trượt lên + hiện dần) / đóng (trượt xuống + mờ đi) như taskbar gốc
    anim: Option<shell::Fade>,
}

/// Quãng trượt của hiệu ứng mở/đóng.
const SLIDE: f32 = shell::FADE_SLIDE;

impl Preview {
    pub fn new(hwnd: HWND) -> Self {
        Preview { hwnd, open: false, tile: 0, cards: Vec::new(), icon: None, theme: None, hover: None, hover_close: false, k: 1.0, x: 0, y: 0, w: 0, h: 0, inside: false, anim: None }
    }

    /// Mở bảng xem trước phía trên điểm (cx, top) – toạ độ màn hình.
    pub fn show(&mut self, tile: isize, wins: &[isize], icon: Option<Pixmap>, cx: i32, top: i32, mon: RECT, k: f32, theme: Option<Theme>) {
        self.close();
        self.finish();
        let maxn = (((mon.right - mon.left) as f32 / k - PAD * 2.0) / (CARD_W + GAP)).floor().max(1.0) as usize;
        let wins: Vec<isize> = wins.iter().copied().take(maxn).collect();
        if wins.is_empty() {
            return;
        }
        self.k = k;
        self.tile = tile;
        self.icon = icon;
        self.theme = theme;
        self.hover = None;
        self.hover_close = false;
        self.inside = false;
        let lw = PAD * 2.0 + wins.len() as f32 * CARD_W + (wins.len() as f32 - 1.0) * GAP;
        let lh = PAD * 2.0 + TITLE_H + THUMB_H;
        let (w, h) = ((lw * k).ceil() as i32, (lh * k).ceil() as i32);
        let x = (cx - w / 2).clamp(mon.left + 4, mon.right - w - 4);
        let y = top - (8.0 * k) as i32 - h;
        self.x = x;
        self.y = y;
        self.w = w;
        self.h = h;
        // Bắt đầu thấp hơn một chút và trong suốt, rồi trượt lên vị trí (xem `animate`)
        shell::set_alpha(self.hwnd, 0);
        shell::move_popup(self.hwnd, x, y + (SLIDE * k) as i32, w, h);
        self.anim = shell::Fade::open();
        self.cards = wins.iter().map(|&win| Card { win, title: shell::window_title(win), thumb: None }).collect();
        self.draw();
        shell::show(self.hwnd, true);
        self.open = true;
        // Ảnh thu nhỏ trực tiếp của từng cửa sổ (Windows tự cập nhật)
        for (i, c) in self.cards.iter_mut().enumerate() {
            let (cx0, cy0) = (PAD + i as f32 * (CARD_W + GAP) + 8.0, PAD + TITLE_H);
            let area = RECT { left: (cx0 * k) as i32, top: (cy0 * k) as i32, right: ((cx0 + CARD_W - 16.0) * k) as i32, bottom: ((cy0 + THUMB_H - 8.0) * k) as i32 };
            c.thumb = shell::thumb_register(self.hwnd, HWND(c.win as *mut _), area);
        }
        shell::watch_outside(self.hwnd, true);
    }

    pub fn animating(&self) -> bool {
        self.anim.is_some()
    }

    /// Chạy một bước hiệu ứng mở/đóng. Trả về true nếu còn đang chạy.
    pub fn animate(&mut self) -> bool {
        let Some(f) = self.anim else { return false };
        let (a, off, done) = shell::fade_state(&f);
        shell::move_window(self.hwnd, self.x, self.y + (off * SLIDE * self.k).round() as i32);
        shell::set_alpha(self.hwnd, (a * 255.0).round() as u8);
        if done {
            self.anim = None;
            if f.closing {
                self.finish();
            }
            return false;
        }
        true
    }

    /// Đóng bảng: trượt xuống + mờ đi rồi mới ẩn hẳn (xem `animate`).
    pub fn close(&mut self) {
        if !self.open {
            return;
        }
        self.open = false;
        self.inside = false;
        shell::watch_outside(self.hwnd, false);
        self.anim = shell::Fade::close();
    }

    /// Ẩn hẳn ngay và bỏ các ảnh thu nhỏ.
    fn finish(&mut self) {
        self.anim = None;
        for c in self.cards.drain(..) {
            if let Some(t) = c.thumb {
                shell::thumb_unregister(t);
            }
        }
        shell::show(self.hwnd, false);
    }

    fn card_at(&self, x: i32, y: i32) -> Option<(usize, bool)> {
        let (lx, ly) = (x as f32 / self.k, y as f32 / self.k);
        if ly < PAD || ly > PAD + TITLE_H + THUMB_H {
            return None;
        }
        let i = ((lx - PAD) / (CARD_W + GAP)).floor();
        if i < 0.0 || i as usize >= self.cards.len() {
            return None;
        }
        let x0 = PAD + i * (CARD_W + GAP);
        if lx > x0 + CARD_W {
            return None;
        }
        let close = lx >= x0 + CARD_W - 32.0 && ly <= PAD + TITLE_H;
        Some((i as usize, close))
    }

    pub fn mouse(&mut self, e: shell::PopupEvent) -> Option<Cmd> {
        if !self.open {
            return None;
        }
        match e {
            shell::PopupEvent::Move(x, y) => {
                self.inside = true;
                let h = self.card_at(x, y);
                let (hv, hc) = (h.map(|v| v.0), h.is_some_and(|v| v.1));
                if hv != self.hover || hc != self.hover_close {
                    self.hover = hv;
                    self.hover_close = hc;
                    self.draw();
                }
            }
            shell::PopupEvent::Leave => {
                self.inside = false;
                if self.hover.take().is_some() {
                    self.draw();
                }
            }
            shell::PopupEvent::Up { right: false } => {
                if let Some(i) = self.hover {
                    let win = self.cards[i].win;
                    let close = self.hover_close;
                    self.close();
                    return Some(if close { Cmd::Close(win) } else { Cmd::Activate(win) });
                }
            }
            shell::PopupEvent::Up { right: true } => {}
            shell::PopupEvent::Outside => self.close(),
        }
        None
    }

    fn draw(&mut self) {
        let Some(f) = super::menu::ui_fonts() else { return };
        let k = self.k;
        let (lw, lh) = (self.w as f32 / k, self.h as f32 / k);
        let mut cv = Canvas::new(self.w as u32, self.h as u32, Transform::from_scale(k, k));
        let (fg, acc, hover) = match self.theme {
            Some(t) => {
                cv.rect(0.0, 0.0, lw, lh, solid(ca(16, 18, 26, 1.0)));
                cv.rect(0.0, 0.0, lw, lh, solid(ca(t.tint.0, t.tint.1, t.tint.2, t.tint.3)));
                if let Some(p) = round_rect_path(0.5, 0.5, lw - 1.0, lh - 1.0, 8.0) {
                    cv.stroke(&p, solid(ca(t.border.0, t.border.1, t.border.2, t.border.3)), 1.0);
                }
                (t.fg, t.acc, ca(255, 255, 255, 0.10))
            }
            None => {
                cv.rect(0.0, 0.0, lw, lh, solid(ca(44, 44, 44, 1.0)));
                ([255, 255, 255], [255, 255, 255], ca(255, 255, 255, 0.07))
            }
        };
        let mut texts: Vec<(bool, f32, f32, f32, [u8; 3], f32, String)> = Vec::new();
        for (i, c) in self.cards.iter().enumerate() {
            let x0 = PAD + i as f32 * (CARD_W + GAP);
            if self.hover == Some(i) {
                cv.round_rect(x0, PAD, CARD_W, TITLE_H + THUMB_H, 6.0, solid(hover));
            }
            // tiêu đề: icon app + tên cửa sổ, nút đóng khi rê chuột
            let tmax = CARD_W - 8.0 - 26.0 - if self.hover == Some(i) { 34.0 } else { 8.0 };
            let t = clip(&f.text, 12.0 * k, &c.title, tmax * k);
            texts.push((false, 12.0, x0 + 34.0, PAD + 20.0, fg, 1.0, t));
            if self.hover == Some(i) {
                let bx = x0 + CARD_W - 30.0;
                if self.hover_close {
                    cv.round_rect(bx, PAD + 3.0, 26.0, 24.0, 4.0, solid(ca(196, 43, 28, 1.0)));
                }
                if f.icons.is_some() {
                    texts.push((true, 10.0, bx + 8.0, PAD + 20.0, if self.hover_close { [255, 255, 255] } else { fg }, 1.0, '\u{E8BB}'.to_string()));
                }
            }
            // ô ảnh thu nhỏ (Windows vẽ đè lên); cửa sổ thu nhỏ thì hiện icon to ở giữa
            if shell::is_minimized(c.win) {
                if let Some(ic) = &self.icon {
                    let s = (40.0 * k) / ic.width().max(1) as f32;
                    let (ix, iy) = ((x0 + CARD_W / 2.0 - 20.0) * k, (PAD + TITLE_H + THUMB_H / 2.0 - 24.0) * k);
                    cv.pm.draw_pixmap(0, 0, ic.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, Transform::from_translate(ix.round(), iy.round()).pre_scale(s, s), None);
                }
            }
        }
        let _ = acc;
        let mut pm = cv.pm;
        if let Some(ic) = &self.icon {
            for i in 0..self.cards.len() {
                let x0 = PAD + i as f32 * (CARD_W + GAP);
                let s = (16.0 * k) / ic.width().max(1) as f32;
                pm.draw_pixmap(0, 0, ic.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, Transform::from_translate(((x0 + 10.0) * k).round(), ((PAD + 7.0) * k).round()).pre_scale(s, s), None);
            }
        }
        for (icon, px, x, bl, col, a, s) in &texts {
            let font = if *icon {
                match &f.icons {
                    Some(i) => i,
                    None => continue,
                }
            } else {
                &f.text
            };
            widget::text(&mut pm, font, px * k, x * k, bl * k, *col, *a, s);
        }
        shell::preview_paint(self.hwnd, self.w, self.h, pm.data());
    }
}

fn clip(font: &ab_glyph::FontVec, px: f32, s: &str, max: f32) -> String {
    if widget::measure(font, px, s) <= max {
        return s.to_string();
    }
    let mut out = s.to_string();
    while !out.is_empty() && widget::measure(font, px, &(out.clone() + "…")) > max {
        out.pop();
    }
    out + "…"
}
