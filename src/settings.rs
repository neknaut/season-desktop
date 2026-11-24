//! Giao diện chọn chủ đề theo màn hình và cấu hình chung.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Align, Color32, FontFamily, FontId, Layout, RichText, Sense, Stroke, Ui};
use winit::event::WindowEvent;
use winit::window::Window;

use crate::config::Config;
use crate::gfx::Gpu;
use crate::scenes::Kind;

const BG: Color32 = Color32::from_rgb(0x0f, 0x11, 0x16);
const PANEL: Color32 = Color32::from_rgb(0x16, 0x19, 0x21);
const LINE: Color32 = Color32::from_rgb(0x26, 0x2a, 0x35);
const FG: Color32 = Color32::from_rgb(0xf4, 0xf2, 0xee);
// chữ phụ: đủ sáng để đọc rõ trên nền tối (tương phản ≥ 7:1)
const MUTED: Color32 = Color32::from_rgb(0xbd, 0xc2, 0xce);

/// Thông tin một màn hình để hiện trong cài đặt.
pub struct MonInfo {
    pub name: String,
    /// "Màn hình 1"
    pub title: String,
    /// "1920×1080 · 120 Hz"
    pub detail: String,
    pub primary: bool,
    pub portrait: bool,
    pub hz: u32,
    /// Cảnh đang hiển thị (None = hình nền của Windows)
    pub wall: Option<Kind>,
    /// Đang dùng taskbar của app
    pub bar: bool,
    /// Chủ đề đang áp dụng cho hình nền / taskbar
    pub applied_theme: String,
    pub applied_bar_theme: String,
}

pub struct Status {
    pub message: String,
    pub message_for: String,
    pub paused_user: bool,
    pub paused_auto: bool,
    pub autostart: bool,
    pub on_battery: bool,
    pub has_battery: bool,
    pub gpu: String,
    /// Chế độ tự động hôm nay chọn gì
    pub auto_kind: Kind,
    pub next_event: Option<(Kind, i64)>,
    pub mons: Vec<MonInfo>,
    /// Cài đặt chung có thay đổi chưa áp dụng
    pub global_dirty: bool,
}

/// Mỗi hành động chỉ tác động lên đúng màn hình (tên thiết bị) được chọn.
pub enum Action {
    ApplyTheme { monitor: String, theme: String },
    ResetTheme(String),
    ApplyGlobal,
    Quit,
    TogglePause,
    OpenFolder,
    Autostart(bool),
}

fn accent(kind: Kind) -> Color32 {
    match kind {
        Kind::Noel => Color32::from_rgb(0x7f, 0xe0, 0xc4),
        Kind::Tet => Color32::from_rgb(0xff, 0xcf, 0x5a),
        Kind::Moon => Color32::from_rgb(0xff, 0xd5, 0x8a),
        k => {
            // Màu nhấn của chủ đề có khi khá tối (vốn dùng trên nền trời sáng): pha sáng lên
            // cho tới khi đủ nổi trên nền tối của cửa sổ, chữ tối trên nút vẫn đọc rõ.
            let a = k.theme().acc;
            let (mut r, mut g, mut b) = (a[0] as f32, a[1] as f32, a[2] as f32);
            let lum = |r: f32, g: f32, b: f32| (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0;
            while lum(r, g, b) < 0.62 {
                r += (255.0 - r) * 0.12;
                g += (255.0 - g) * 0.12;
                b += (255.0 - b) * 0.12;
            }
            Color32::from_rgb(r as u8, g as u8, b as u8)
        }
    }
}

pub struct SettingsWin {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    pub next_repaint: Option<Instant>,
    /// Màn hình đang chọn trong cài đặt
    sel: usize,
    /// Ảnh xem trước từng chủ đề: (cảnh, dọc?, ảnh)
    previews: Vec<(Kind, bool, egui::TextureHandle)>,
    /// Ảnh xem trước giải mã ở luồng nền (mở cửa sổ không bị khựng); nhận dần từng ảnh
    preview_rx: Option<std::sync::mpsc::Receiver<(Kind, bool, Vec<u8>, u32, u32)>>,
    /// Lúc mở cửa sổ (màn hình chờ hiện tối đa vài giây)
    opened: Instant,
    /// Icon app (dùng làm logo ở góc trên)
    logo: Option<egui::TextureHandle>,
}

impl SettingsWin {
    pub fn new(gpu: &Gpu, window: Arc<Window>, surface: Option<wgpu::Surface<'static>>) -> Result<Self, String> {
        let surface = match surface {
            Some(s) => s,
            None => gpu.instance.create_surface(window.clone()).map_err(|e| e.to_string())?,
        };
        let size = window.inner_size();
        let config = gpu.surface_config(&surface, size.width, size.height);
        surface.configure(&gpu.device, &config);

        let ctx = egui::Context::default();
        // Cùng font với bản xem thử: Be Vietnam Pro.
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert("bvp".into(), Arc::new(egui::FontData::from_static(crate::widget::BE_VIETNAM_REGULAR)));
        fonts.font_data.insert("bvp-semibold".into(), Arc::new(egui::FontData::from_static(crate::widget::BE_VIETNAM_SEMIBOLD)));
        if let Some(f) = fonts.families.get_mut(&FontFamily::Proportional) {
            f.insert(0, "bvp".into());
        }
        fonts.families.insert(FontFamily::Name("bold".into()), vec!["bvp-semibold".into(), "bvp".into()]);
        ctx.set_fonts(fonts);

        let mut v = egui::Visuals::dark();
        v.panel_fill = BG;
        v.window_fill = BG;
        v.extreme_bg_color = Color32::from_rgb(0x0b, 0x0d, 0x12);
        v.widgets.inactive.bg_fill = LINE;
        v.widgets.inactive.weak_bg_fill = LINE;
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x30, 0x35, 0x42);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, FG);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, FG);
        v.slider_trailing_fill = true;
        ctx.set_visuals(v);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(10.0_f32, 8.0);
            s.spacing.button_padding = egui::vec2(14.0_f32, 7.0);
            s.spacing.slider_width = 170.0;
            s.spacing.interact_size.y = 24.0;
        });

        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, &window, Some(window.scale_factor() as f32), None, None);
        let renderer = egui_wgpu::Renderer::new(&gpu.device, config.format, None, 1, false);
        window.request_redraw();
        let logo = {
            let (rgba, n) = crate::tray::icon_rgba(96);
            Some(ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([n as usize, n as usize], &rgba), egui::TextureOptions::LINEAR))
        };
        // Giải mã ảnh xem trước của mọi chủ đề (cả ngang lẫn dọc) ở luồng nền
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = std::thread::Builder::new().name("anh-xem-truoc".into()).spawn(move || {
            for portrait in [false, true] {
                for kind in Kind::ALL {
                    if let Some(pm) = crate::scenes::thumbnail(kind, portrait) {
                        if tx.send((kind, portrait, pm.data().to_vec(), pm.width(), pm.height())).is_err() {
                            return;
                        }
                    }
                }
            }
        });
        Ok(Self { window, surface, config, ctx, state, renderer, next_repaint: None, sel: 0, previews: Vec::new(), preview_rx: Some(rx), opened: Instant::now(), logo })
    }

    pub fn on_event(&mut self, gpu: &Gpu, ev: &WindowEvent) {
        if let WindowEvent::Resized(s) = ev {
            if s.width > 0 && s.height > 0 {
                self.config.width = s.width;
                self.config.height = s.height;
                self.surface.configure(&gpu.device, &self.config);
            }
        }
        let r = self.state.on_window_event(&self.window, ev);
        if r.repaint {
            self.window.request_redraw();
        }
    }

    /// Nhận các ảnh xem trước đã giải mã xong ở luồng nền và đưa lên GPU.
    fn take_previews(&mut self) {
        let Some(rx) = &self.preview_rx else { return };
        loop {
            match rx.try_recv() {
                Ok((kind, portrait, data, w, h)) => {
                    let image = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &data);
                    let texture = self.ctx.load_texture(format!("{kind:?}-{portrait}"), image, egui::TextureOptions::LINEAR);
                    self.previews.push((kind, portrait, texture));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.preview_rx = None;
                    break;
                }
            }
        }
    }

    pub fn paint(&mut self, gpu: &Gpu, cfg: &mut Config, st: &Status) -> Vec<Action> {
        self.take_previews();
        // Màn hình chờ: tới khi đủ ảnh của hướng màn hình đang xem (tối đa 3 giây)
        let portrait = st.mons.get(self.sel).map(|m| m.portrait).unwrap_or(false);
        let ready = self.previews.iter().filter(|p| p.1 == portrait).count();
        let loading = (self.preview_rx.is_some() && ready < Kind::ALL.len() && self.opened.elapsed() < Duration::from_secs(3))
            .then_some((ready, Kind::ALL.len()));
        if self.preview_rx.is_some() {
            // còn ảnh đang giải mã: vẽ lại sớm để hiện dần
            self.ctx.request_repaint_after(Duration::from_millis(30));
        }
        let raw = self.state.take_egui_input(&self.window);
        let mut actions = Vec::new();
        let sel = &mut self.sel;
        let previews = &self.previews;
        let logo = self.logo.as_ref();
        let previous_selection = *sel;
        let out = self.ctx.run(raw, |ctx| match loading {
            Some(progress) => loading_screen(ctx, logo, progress),
            None => ui(ctx, cfg, st, &mut actions, sel, previews, logo),
        });
        if *sel != previous_selection {
            self.window.request_redraw();
        }
        self.state.handle_platform_output(&self.window, out.platform_output);
        let ppp = out.pixels_per_point;
        let tris = self.ctx.tessellate(out.shapes, ppp);
        for (id, delta) in &out.textures_delta.set {
            self.renderer.update_texture(&gpu.device, &gpu.queue, *id, delta);
        }
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&gpu.device, &self.config);
                self.window.request_redraw();
                return actions;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [self.config.width, self.config.height], pixels_per_point: ppp };
        let extra = self.renderer.update_buffers(&gpu.device, &gpu.queue, &mut enc, &tris, &sd);
        {
            let mut pass = enc
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("settings"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.059, g: 0.067, b: 0.086, a: 1.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &tris, &sd);
        }
        gpu.queue.submit(extra.into_iter().chain([enc.finish()]));
        frame.present();
        for id in &out.textures_delta.free {
            self.renderer.free_texture(id);
        }
        let delay = out.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay).unwrap_or(Duration::MAX);
        self.next_repaint = if delay < Duration::from_secs(3600) { Some(Instant::now() + delay) } else { None };
        actions
    }
}

// ---------------------------------------------------------------- giao diện

fn row(ui: &mut Ui, label: &str, sub: Option<&str>, add: impl FnOnce(&mut Ui)) {
    ui.add_space(5.0);
    ui.horizontal(|ui| {
        let label_width = (ui.available_width() - 174.0).max(120.0);
        ui.allocate_ui_with_layout(egui::vec2(label_width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(label).size(15.0).color(FG));
            if let Some(text) = sub {
                ui.label(RichText::new(text).size(13.0).color(MUTED));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), add);
    });
    ui.add_space(5.0);
}

fn toggle(ui: &mut Ui, on: &mut bool, acc: Color32) -> bool {
    let size = egui::vec2(42.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let bg = if *on { acc } else { LINE };
    let p = ui.painter();
    p.rect_filled(rect, 12.0, bg);
    let x = egui::lerp((rect.left() + 12.0)..=(rect.right() - 12.0), t);
    p.circle_filled(egui::pos2(x, rect.center().y), 9.0, if *on { BG } else { FG });
    resp.clicked()
}

/// Thẻ chọn chủ đề: ảnh xem trước thật của cảnh ở trên, tên và mô tả ở dưới.
fn theme_card(ui: &mut Ui, width: f32, portrait: bool, selected: bool, title: &str, sub: &str, texture: Option<&egui::TextureHandle>, accent: Color32) -> bool {
    let pad = 8.0;
    let img_width = width - pad * 2.0;
    let img_height = img_width * if portrait { 16.0 / 9.0 } else { 9.0 / 16.0 };
    let (card, response) = ui.allocate_exact_size(egui::vec2(width, img_height + pad + 72.0), Sense::click());
    let painter = ui.painter();
    let hovered = response.hovered();
    let line = if selected { accent } else if hovered { accent.gamma_multiply(0.55) } else { LINE };
    painter.rect_filled(card, 13.0, line);
    painter.rect_filled(card.shrink(if selected { 2.0 } else { 1.0 }), 12.0, if hovered { HOVER } else { PANEL });
    let image = egui::Rect::from_min_size(card.min + egui::vec2(pad, pad), egui::vec2(img_width, img_height));
    match texture {
        Some(texture) => cover_image(painter, image, 8.0, texture),
        None => {
            // ảnh chưa nạp xong: khung chờ nhấp nháy nhẹ
            let t = ui.input(|i| i.time) as f32;
            let a = (0.5 + 0.5 * (t * 3.0).sin()) * 0.06;
            painter.rect_filled(image, 8.0, SIDEBAR);
            painter.rect_filled(image, 8.0, Color32::from_white_alpha((a * 255.0) as u8));
            ui.ctx().request_repaint();
        }
    }
    if selected {
        // nhãn "Đang dùng" ở góc ảnh
        let galley = painter.layout_no_wrap("Đang dùng".into(), bold(11.5), BG);
        let b = egui::Rect::from_min_size(image.min + egui::vec2(8.0, 8.0), egui::vec2(galley.size().x + 26.0, 22.0));
        painter.rect_filled(b, 11.0, accent);
        let c = egui::pos2(b.left() + 11.0, b.center().y);
        let stroke = Stroke::new(1.8_f32, BG);
        painter.line_segment([c + egui::vec2(-3.5, 0.0), c + egui::vec2(-1.0, 2.5)], stroke);
        painter.line_segment([c + egui::vec2(-1.0, 2.5), c + egui::vec2(3.5, -2.5)], stroke);
        painter.galley(egui::pos2(b.left() + 19.0, b.center().y - galley.size().y / 2.0), galley, BG);
    }

    let x = card.left() + 12.0;
    let y = image.bottom() + 10.0;
    let title_area = egui::Rect::from_min_max(egui::pos2(x, y), egui::pos2(card.right() - 12.0, y + 18.0));
    painter.with_clip_rect(title_area).text(title_area.min, egui::Align2::LEFT_TOP, title, bold(14.5), FG);
    let body = egui::Rect::from_min_max(egui::pos2(x, y + 23.0), egui::pos2(card.right() - 12.0, card.bottom() - 6.0));
    let description = painter.layout(sub.to_owned(), FontId::proportional(12.5), MUTED, body.width());
    painter.with_clip_rect(body).galley(body.min, description, MUTED);

    response.on_hover_text("Áp dụng hình nền và taskbar").clicked()
}

fn primary(ui: &mut Ui, text: &str, acc: Color32) -> bool {
    ui.add(egui::Button::new(RichText::new(text).size(14.0).color(BG).family(FontFamily::Name("bold".into()))).fill(acc).corner_radius(8.0).min_size(egui::vec2(0.0, 34.0)))
        .clicked()
}

fn secondary(ui: &mut Ui, text: &str) -> bool {
    ui.add(egui::Button::new(RichText::new(text).size(13.5).color(FG)).fill(LINE).corner_radius(8.0).min_size(egui::vec2(0.0, 34.0))).clicked()
}

fn kind_for(theme: &str, auto: Kind) -> Kind {
    Kind::from_id(theme).unwrap_or(auto)
}

/// Hàng nút chọn sẵn (ví dụ các mức FPS); mức đề xuất có ghi chú.
fn chips(ui: &mut Ui, value: &mut u32, opts: &[(u32, String)], rec: u32, acc: Color32) {
    ui.horizontal_wrapped(|ui| {
        for (v, label) in opts {
            let sel = *value == *v;
            let text = if *v == rec { format!("{label} · {v} fps · đề xuất") } else { format!("{label} · {v} fps") };
            let b = egui::Button::new(RichText::new(text).size(13.0).color(if sel { BG } else { FG }))
                .fill(if sel { acc } else { LINE })
                .corner_radius(16.0)
                .min_size(egui::vec2(0.0, 30.0));
            if ui.add(b).clicked() {
                *value = *v;
            }
        }
    });
}

const SIDEBAR: Color32 = Color32::from_rgb(0x12, 0x14, 0x1b);
const HOVER: Color32 = Color32::from_rgb(0x1d, 0x21, 0x2b);
const SELECT: Color32 = Color32::from_rgb(0x23, 0x28, 0x34);

fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Một mục màn hình ở cột trái: biểu tượng ngang/dọc, tên, độ phân giải, nhãn "Chính".
fn monitor_item(ui: &mut Ui, m: &MonInfo, selected: bool, thumb: Option<&egui::TextureHandle>) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 60.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 10.0, SELECT);
    } else if resp.hovered() {
        p.rect_filled(rect, 10.0, HOVER);
    }
    // biểu tượng màn hình (đúng hướng ngang/dọc), tô màu chủ đề nếu đang có hình nền
    let fill = m.wall.map(accent).unwrap_or(LINE);
    let (gw, gh) = if m.portrait { (18.0, 28.0) } else { (32.0, 20.0) };
    let c = egui::pos2(rect.left() + 28.0, rect.center().y - 2.0);
    let g = egui::Rect::from_center_size(c, egui::vec2(gw, gh));
    p.rect_filled(g.expand(2.0), 4.0, Color32::from_rgb(0x3a, 0x40, 0x4f));
    match thumb {
        // ảnh thu nhỏ của chủ đề đang chạy trên màn hình đó
        Some(t) => cover_image(p, g, 3.0, t),
        None => {
            p.rect_filled(g, 3.0, fill);
        }
    }
    p.rect_filled(egui::Rect::from_center_size(egui::pos2(c.x, g.bottom() + 5.0), egui::vec2(10.0, 2.5)), 1.0, Color32::from_rgb(0x3a, 0x40, 0x4f));
    if m.bar {
        // vạch nhỏ dưới đáy = đang dùng taskbar của app
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(g.left() + 3.0, g.bottom() - 4.0), egui::vec2(g.width() - 6.0, 2.0)), 1.0, BG);
    }
    p.text(egui::pos2(rect.left() + 54.0, rect.top() + 12.0), egui::Align2::LEFT_TOP, &m.title, bold(14.5), FG);
    p.text(egui::pos2(rect.left() + 54.0, rect.top() + 33.0), egui::Align2::LEFT_TOP, &m.detail, FontId::proportional(12.5), MUTED);
    if m.primary {
        let galley = p.layout_no_wrap("Chính".into(), FontId::proportional(11.0), FG);
        let bw = galley.size().x + 12.0;
        let b = egui::Rect::from_min_size(egui::pos2(rect.right() - bw - 10.0, rect.top() + 12.0), egui::vec2(bw, 18.0));
        p.rect_filled(b, 9.0, LINE);
        p.galley(egui::pos2(b.left() + 6.0, b.top() + 2.0), galley, FG);
    }
    resp.clicked()
}

fn nav_item(ui: &mut Ui, title: &str, sub: &str, selected: bool) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 52.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 10.0, SELECT);
    } else if resp.hovered() {
        p.rect_filled(rect, 10.0, HOVER);
    }
    // biểu tượng bánh răng đơn giản
    let c = egui::pos2(rect.left() + 28.0, rect.center().y);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        p.circle_filled(egui::pos2(c.x + a.cos() * 9.0, c.y + a.sin() * 9.0), 2.6, MUTED);
    }
    p.circle_filled(c, 7.5, MUTED);
    p.circle_filled(c, 3.5, if selected { SELECT } else { SIDEBAR });
    p.text(egui::pos2(rect.left() + 54.0, rect.top() + 9.0), egui::Align2::LEFT_TOP, title, bold(14.5), FG);
    p.text(egui::pos2(rect.left() + 54.0, rect.top() + 29.0), egui::Align2::LEFT_TOP, sub, FontId::proportional(12.5), MUTED);
    resp.clicked()
}

/// Ô trạng thái nhỏ (chấm màu + chữ).
fn status_chip(ui: &mut Ui, text: &str, dot: Color32) {
    egui::Frame::default().fill(PANEL).stroke(Stroke::new(1.0_f32, LINE)).corner_radius(14.0).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, dot);
            ui.label(RichText::new(text).size(12.5).color(FG));
        });
    });
}

/// Khối có tiêu đề, mô tả ngắn và nội dung.
fn block(ui: &mut Ui, title: &str, sub: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(16.0);
    egui::Frame::default().fill(PANEL).stroke(Stroke::new(1.0_f32, LINE)).corner_radius(12.0).inner_margin(18.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).font(bold(16.0)).color(FG));
        if !sub.is_empty() {
            ui.label(RichText::new(sub).size(13.0).color(MUTED));
        }
        ui.add_space(10.0);
        add(ui)
    });
}

fn note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(12.5).color(Color32::from_rgb(0xe8, 0xc0, 0x6a)));
}

/// Màn hình chờ lúc mở cửa sổ: logo, tên app, vòng quay và thanh tiến độ nạp chủ đề.
fn loading_screen(ctx: &egui::Context, logo: Option<&egui::TextureHandle>, (done, total): (usize, usize)) {
    let acc = Color32::from_rgb(0x8f, 0xd3, 0xff);
    egui::CentralPanel::default().frame(egui::Frame::default().fill(BG)).show(ctx, |ui| {
        let r = ui.max_rect();
        let c = r.center();
        let p = ui.painter();
        // quầng sáng nhẹ sau logo
        for (rr, a) in [(140.0f32, 10u8), (95.0, 16), (60.0, 22)] {
            p.circle_filled(c - egui::vec2(0.0, 40.0), rr, Color32::from_rgba_unmultiplied(0x8f, 0xd3, 0xff, a));
        }
        let lr = egui::Rect::from_center_size(c - egui::vec2(0.0, 40.0), egui::vec2(72.0, 72.0));
        match logo {
            Some(t) => {
                p.image(t.id(), lr, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                p.rect_filled(lr, 16.0, acc);
            }
        }
        p.text(c + egui::vec2(0.0, 18.0), egui::Align2::CENTER_TOP, "Season Desktop", bold(22.0), FG);
        p.text(c + egui::vec2(0.0, 50.0), egui::Align2::CENTER_TOP, format!("Đang tải chủ đề {done}/{total}"), FontId::proportional(13.0), MUTED);
        // thanh tiến độ
        let bar = egui::Rect::from_center_size(c + egui::vec2(0.0, 86.0), egui::vec2(220.0, 4.0));
        p.rect_filled(bar, 2.0, LINE);
        let f = if total == 0 { 0.0 } else { done as f32 / total as f32 };
        p.rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * f.max(0.04), bar.height())), 2.0, acc);
        // vòng quay
        let t = ui.input(|i| i.time) as f32;
        let sc = c + egui::vec2(0.0, 118.0);
        for k in 0..8 {
            let a = k as f32 / 8.0 * std::f32::consts::TAU + t * 5.0;
            let alpha = ((k as f32 + 1.0) / 8.0 * 200.0) as u8;
            p.circle_filled(sc + egui::vec2(a.cos(), a.sin()) * 9.0, 2.0, Color32::from_rgba_unmultiplied(0x8f, 0xd3, 0xff, alpha));
        }
    });
    ctx.request_repaint();
}

/// Ảnh xem trước của một chủ đề theo hướng màn hình.
fn preview_of<'a>(previews: &'a [(Kind, bool, egui::TextureHandle)], kind: Kind, portrait: bool) -> Option<&'a egui::TextureHandle> {
    previews.iter().find(|(k, p, _)| *k == kind && *p == portrait).map(|(_, _, t)| t)
}

/// Vẽ ảnh phủ kín khung `rect` (cắt bớt phần thừa, giữ tỉ lệ), bo góc.
fn cover_image(p: &egui::Painter, rect: egui::Rect, radius: f32, tex: &egui::TextureHandle) {
    let [tw, th] = tex.size();
    let (ta, ra) = (tw as f32 / th.max(1) as f32, rect.width() / rect.height().max(1.0));
    let uv = if ta > ra {
        let w = ra / ta;
        egui::Rect::from_min_max(egui::pos2((1.0 - w) / 2.0, 0.0), egui::pos2((1.0 + w) / 2.0, 1.0))
    } else {
        let h = ta / ra;
        egui::Rect::from_min_max(egui::pos2(0.0, (1.0 - h) / 2.0), egui::pos2(1.0, (1.0 + h) / 2.0))
    };
    p.add(egui::epaint::RectShape::filled(rect, radius, Color32::WHITE).with_texture(tex.id(), uv));
}

fn theme_label(id: &str) -> &'static str {
    Kind::from_id(id).map(|k| k.name()).unwrap_or("Tự động")
}

fn ui(ctx: &egui::Context, cfg: &mut Config, st: &Status, actions: &mut Vec<Action>, sel: &mut usize, previews: &[(Kind, bool, egui::TextureHandle)], logo: Option<&egui::TextureHandle>) {
    if *sel > st.mons.len() {
        *sel = 0;
    }
    let general = *sel == st.mons.len();
    let cur_kind = st.mons.get(*sel).map(|m| kind_for(&cfg.mon(&m.name, m.primary).theme, st.auto_kind)).unwrap_or(st.auto_kind);
    let acc = accent(cur_kind);
    ctx.style_mut(|s| {
        s.visuals.selection.bg_fill = acc;
        s.visuals.selection.stroke = Stroke::new(1.0_f32, acc);
        s.visuals.widgets.active.bg_fill = acc;
    });

    // ---------------------------------------------------------------- cột trái
    egui::SidePanel::left("monitors")
        .exact_width(232.0)
        .resizable(false)
        .frame(egui::Frame::default().fill(SIDEBAR).inner_margin(egui::Margin::symmetric(14, 18)).stroke(Stroke::new(1.0_f32, LINE)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), Sense::hover());
                match logo {
                    // Icon app thật (cùng icon với exe, khay, cửa sổ)
                    Some(t) => {
                        ui.put(r, egui::Image::new((t.id(), r.size())));
                    }
                    None => {
                        ui.painter().rect_filled(r, 8.0, acc);
                    }
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new("Season Desktop").font(bold(16.0)).color(FG));
                    ui.label(RichText::new("Hình nền & taskbar theo mùa").size(12.0).color(MUTED));
                });
            });
            ui.add_space(20.0);
            ui.label(RichText::new(format!("MÀN HÌNH ({})", st.mons.len())).size(11.5).color(MUTED).extra_letter_spacing(1.2));
            ui.add_space(6.0);
            for (i, m) in st.mons.iter().enumerate() {
                let thumb = m.wall.and_then(|k| preview_of(previews, k, false));
                if monitor_item(ui, m, *sel == i, thumb) {
                    *sel = i;
                }
                ui.add_space(4.0);
            }
            ui.add_space(14.0);
            ui.label(RichText::new("ỨNG DỤNG").size(11.5).color(MUTED).extra_letter_spacing(1.2));
            ui.add_space(6.0);
            if nav_item(ui, "Cài đặt chung", "Hiệu ứng, hiệu năng", general) {
                *sel = st.mons.len();
            }

            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                if ui
                    .add(egui::Button::new(RichText::new("Thoát Season Desktop").size(13.0).color(Color32::from_rgb(0xf0, 0x9a, 0x8c))).fill(PANEL).corner_radius(8.0).min_size(egui::vec2(ui.available_width(), 32.0)))
                    .clicked()
                {
                    actions.push(Action::Quit);
                }
                ui.add_space(8.0);
                let (txt, col) = if st.paused_user {
                    ("Hình nền đang tạm dừng", MUTED)
                } else if st.paused_auto {
                    ("Đang nghỉ (bị che kín)", Color32::from_rgb(0xe8, 0xc0, 0x6a))
                } else {
                    ("Đang chạy", Color32::from_rgb(0x7f, 0xe0, 0xc4))
                };
                status_chip(ui, txt, col);
            });
        });

    // ---------------------------------------------------------------- cột phải
    egui::CentralPanel::default().frame(egui::Frame::default().fill(BG).inner_margin(egui::Margin::symmetric(22, 18))).show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.set_width(ui.available_width() - 12.0);
            if general {
                general_page(ui, cfg, st, actions, acc);
            } else if let Some(m) = st.mons.get(*sel) {
                monitor_page(ui, st, m, actions, acc, previews);
            }
            ui.add_space(10.0);
        });
    });
}

fn monitor_page(ui: &mut Ui, st: &Status, m: &MonInfo, actions: &mut Vec<Action>, acc: Color32, previews: &[(Kind, bool, egui::TextureHandle)]) {
    let applied = m.wall.is_some() && m.bar && m.applied_theme == m.applied_bar_theme;
    let is_auto = applied && m.applied_theme == "auto";
    let orientation = if m.portrait { "Màn hình dọc" } else { "Màn hình ngang" };

    // ---- băng ảnh đầu trang: chủ đề đang chạy, trạng thái và thao tác nhanh
    let w = ui.available_width();
    let (hero, _) = ui.allocate_exact_size(egui::vec2(w, 196.0), Sense::hover());
    {
        let p = ui.painter();
        let shown = m.wall.unwrap_or(st.auto_kind);
        match preview_of(previews, shown, false) {
            Some(t) => cover_image(p, hero, 16.0, t),
            None => {
                p.rect_filled(hero, 16.0, PANEL);
            }
        }
        // phủ tối để chữ luôn đọc rõ trên mọi ảnh
        p.rect_filled(hero, 16.0, Color32::from_rgba_unmultiplied(8, 10, 16, if m.wall.is_some() { 150 } else { 200 }));
        p.rect_stroke(hero, 16.0, Stroke::new(1.0_f32, LINE), egui::StrokeKind::Inside);
        let x = hero.left() + 24.0;
        p.text(egui::pos2(x, hero.top() + 22.0), egui::Align2::LEFT_TOP, &m.title, bold(26.0), FG);
        p.text(egui::pos2(x, hero.top() + 58.0), egui::Align2::LEFT_TOP, format!("{} · {orientation}", m.detail), FontId::proportional(13.0), MUTED);
        let line = match (m.wall, m.bar) {
            (Some(k), true) => format!("Hình nền và taskbar · {}", if is_auto { format!("Tự động ({})", k.name()) } else { k.name().to_string() }),
            (Some(k), false) => format!("Hình nền · {} · taskbar Windows", k.name()),
            (None, true) => format!("Hình nền Windows · taskbar {}", theme_label(&m.applied_bar_theme)),
            (None, false) => "Đang dùng hình nền và taskbar của Windows".to_string(),
        };
        p.text(egui::pos2(x, hero.top() + 84.0), egui::Align2::LEFT_TOP, line, bold(14.5), if m.wall.is_some() { acc } else { FG });
        let mut cal = format!("Hôm nay · {}", st.auto_kind.name());
        if let Some((kind, days)) = st.next_event.filter(|(_, days)| *days > 0) {
            cal += &format!("   ·   Sắp tới · {} · còn {days} ngày", kind.name());
        }
        p.text(egui::pos2(x, hero.top() + 108.0), egui::Align2::LEFT_TOP, cal, FontId::proportional(12.5), MUTED);
    }
    // nút thao tác nằm trong băng ảnh
    let btns = egui::Rect::from_min_max(egui::pos2(hero.left() + 24.0, hero.bottom() - 54.0), egui::pos2(hero.right() - 24.0, hero.bottom() - 16.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(btns), |ui| {
        ui.horizontal(|ui| {
            let caption = if is_auto { "Tự động đang bật" } else { "Bật Tự động" };
            if ui.add_enabled_ui(!is_auto, |ui| primary(ui, caption, acc)).inner {
                actions.push(Action::ApplyTheme { monitor: m.name.clone(), theme: "auto".into() });
            }
            if (m.wall.is_some() || m.bar) && secondary(ui, "Khôi phục gốc") {
                actions.push(Action::ResetTheme(m.name.clone()));
            }
        });
    });

    if !st.message.is_empty() && st.message_for == m.name {
        ui.add_space(10.0);
        ui.label(RichText::new(&st.message).size(13.0).color(acc));
    }

    block(ui, "Bộ sưu tập", "Bấm ảnh để áp dụng cả hình nền và taskbar.", |ui| {
        let filter_id = ui.id().with("theme_filter");
        let mut filter = ui.data(|data| data.get_temp::<usize>(filter_id)).unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            for (index, label) in ["Tất cả", "Bốn mùa", "Ngày lễ"].into_iter().enumerate() {
                let sel = filter == index;
                let text = RichText::new(label).size(13.5).color(if sel { BG } else { FG });
                let text = if sel { text.family(FontFamily::Name("bold".into())) } else { text };
                let b = egui::Button::new(text)
                    .fill(if sel { acc } else { LINE })
                    .stroke(Stroke::NONE)
                    .corner_radius(16.0)
                    .min_size(egui::vec2(84.0, 32.0));
                if ui.add(b).clicked() {
                    filter = index;
                }
            }
        });
        ui.data_mut(|data| data.insert_temp(filter_id, filter));
        ui.add_space(10.0);

        let gap = 16.0;
        let min_card = if m.portrait { 146.0 } else { 186.0 };
        let available = ui.available_width();
        let max_cols = 4;
        let cols = (((available + gap) / (min_card + gap)) as usize).clamp(1, max_cols);
        let card_width = (available - gap * (cols - 1) as f32) / cols as f32;
        let (seasons, holidays) = Kind::ALL.split_at(4);
        for (index, (label, themes)) in [("Bốn mùa", seasons), ("Ngày lễ", holidays)].into_iter().enumerate() {
            if filter != 0 && filter != index + 1 {
                continue;
            }
            ui.label(RichText::new(label).font(bold(13.5)).color(MUTED));
            ui.add_space(5.0);
            for row in themes.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for &kind in row {
                        let tex = previews.iter().find(|(k, portrait, _)| *k == kind && *portrait == m.portrait)
                            .map(|(_, _, texture)| texture);
                        let active = applied && m.applied_theme == kind.id();
                        if theme_card(ui, card_width, m.portrait, active, kind.name(), kind.about(), tex, accent(kind)) {
                            actions.push(Action::ApplyTheme { monitor: m.name.clone(), theme: kind.id().into() });
                        }
                    }
                });
                ui.add_space(gap);
            }
        }
    });
}

fn general_page(ui: &mut Ui, cfg: &mut Config, st: &Status, actions: &mut Vec<Action>, acc: Color32) {
    egui::Frame::default().fill(PANEL).stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(12.0).inner_margin(18.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Cài đặt chung").font(bold(24.0)).color(FG));
            ui.label(RichText::new("Áp dụng cho mọi màn hình.").size(13.0).color(MUTED));
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled_ui(st.global_dirty, |ui| primary(ui, "Áp dụng thay đổi", acc)).inner {
                    actions.push(Action::ApplyGlobal);
                }
                if st.global_dirty {
                    note(ui, "Có thay đổi chưa lưu");
                }
            });
        });
    if !st.message.is_empty() && st.message_for.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new(&st.message).size(13.0).color(acc));
    }

    block(ui, "Hiển thị", "", |ui| {
        row(ui, "Đồng hồ và lời chúc", Some("Hiện thông tin trên hình nền."), |ui| {
            toggle(ui, &mut cfg.clock, acc);
        });
        row(ui, "Giữ hình nền khi thoát", Some("Lưu cảnh hiện tại làm hình nền Windows."), |ui| {
            toggle(ui, &mut cfg.keep_on_exit, acc);
        });
        row(ui, "Khi nhấn X", Some("Thu xuống khay hệ thống (app vẫn chạy) hoặc thoát hẳn app."), |ui| {
            // nút bên phải trước vì hàng xếp từ phải sang trái
            for (on, label) in [(false, "Thoát app"), (true, "Thu xuống khay")] {
                let sel = cfg.close_to_tray == on;
                let text = RichText::new(label).size(13.0).color(if sel { BG } else { FG });
                let b = egui::Button::new(text).fill(if sel { acc } else { LINE }).stroke(Stroke::NONE).corner_radius(14.0).min_size(egui::vec2(0.0, 30.0));
                if ui.add(b).clicked() {
                    cfg.close_to_tray = on;
                }
            }
        });
    });

    block(ui, "Hiệu ứng động", "", |ui| {
        row(ui, "Tốc độ chuyển động", None, |ui| {
            ui.add(egui::Slider::new(&mut cfg.speed, 0.25..=2.0).suffix("×").max_decimals(2));
        });
        row(ui, "Tuyết rơi", Some("Giáng sinh · đặt 0 để tắt."), |ui| {
            ui.add(egui::Slider::new(&mut cfg.snow, 0.0..=2.0).max_decimals(2));
        });
        row(ui, "Pháo hoa", Some("Đêm Giao thừa năm mới."), |ui| {
            toggle(ui, &mut cfg.fireworks, acc);
        });
        row(ui, "Lá thu và hoa mai rơi", Some("Lá rơi mùa thu, hoa mai rơi ngày Tết."), |ui| {
            toggle(ui, &mut cfg.petals, acc);
        });
        row(ui, "Đèn trời", Some("Trung thu."), |ui| {
            toggle(ui, &mut cfg.lanterns, acc);
        });
    });

    block(ui, "Hiệu năng", "", |ui| {
        let max_hz = st.mons.iter().map(|m| m.hz).max().unwrap_or(60);
        let gpu = if st.gpu.is_empty() { "GPU" } else { &st.gpu };
        ui.label(RichText::new(format!("{gpu} · màn hình tối đa {max_hz} Hz")).size(12.5).color(MUTED));
        ui.add_space(8.0);

        let g = st.gpu.to_lowercase();
        let discrete = ["nvidia", "geforce", "rtx", "gtx", "radeon rx", "arc a"].iter().any(|name| g.contains(name));
        let rec = if st.has_battery || !discrete { 30 } else { 60.min(max_hz) };
        ui.label(RichText::new(if st.has_battery { "FPS khi cắm sạc" } else { "Giới hạn FPS" }).size(13.5).color(FG));
        let mut opts = vec![(15, "Tiết kiệm".into()), (30, "Cân bằng".into())];
        if max_hz > 30 {
            opts.push((max_hz.min(60), "Mượt".into()));
        }
        if max_hz > 60 {
            opts.push((max_hz, "Tối đa".into()));
        }
        chips(ui, &mut cfg.fps, &opts, rec, acc);
        if st.has_battery {
            ui.add_space(8.0);
            let label = if st.on_battery { "FPS khi dùng pin · đang dùng pin" } else { "FPS khi dùng pin" };
            ui.label(RichText::new(label).size(13.5).color(FG));
            chips(ui, &mut cfg.battery_fps, &[(10, "Rất tiết kiệm".into()), (15, "Tiết kiệm".into()), (30, "Cân bằng".into())], 15, acc);
        }
        ui.add_space(10.0);
        row(ui, "Tạm dừng khi toàn màn hình", Some("Khi chơi game, xem phim hoặc trình chiếu."), |ui| {
            toggle(ui, &mut cfg.pause_on_fullscreen, acc);
        });
        row(ui, "Tạm dừng khi bị che kín", Some("Khi cửa sổ phóng to che toàn bộ hình nền."), |ui| {
            toggle(ui, &mut cfg.pause_on_maximized, acc);
        });
    });

    block(ui, "Hệ thống", "", |ui| {
        let mut auto = st.autostart;
        row(ui, "Khởi động cùng Windows", Some("Chạy nền khi đăng nhập."), |ui| {
            if toggle(ui, &mut auto, acc) {
                actions.push(Action::Autostart(auto));
            }
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            let label = if st.paused_user { "Chạy tiếp hình nền" } else { "Tạm dừng hình nền" };
            if secondary(ui, label) {
                actions.push(Action::TogglePause);
            }
            if secondary(ui, "Mở thư mục cấu hình") {
                actions.push(Action::OpenFolder);
            }
        });
    });
}
