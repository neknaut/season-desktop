// Vẽ mọi thứ trên hình nền: lớp ảnh tĩnh, ảnh trong atlas, quầng sáng, chấm tròn, hình chữ nhật.
// Toạ độ thiết kế 1280x720 (giống bản xem thử), đổi sang pixel bằng scale + off.

struct G {
    res: vec2f,
    scale: f32,
    p0: f32,
    off: vec2f,
    p1: vec2f,
};
@group(0) @binding(0) var<uniform> g: G;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VO {
    @builtin(position) p: vec4f,
    @location(0) lc: vec2f,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) color: vec4f,
    @location(3) uv: vec2f,
    @location(4) aa: f32,
    // kích thước thật (pixel) của hình chữ nhật, để khử răng cưa theo độ phủ điểm ảnh
    @location(5) pxs: vec2f,
};

@vertex
fn vs(
    @builtin(vertex_index) vi: u32,
    @location(0) pos: vec2f,
    @location(1) size: vec2f,
    @location(2) rot: f32,
    @location(3) kind: u32,
    @location(4) color: vec4f,
    @location(5) uvr: vec4f,
) -> VO {
    var corners = array<vec2f, 6>(
        vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(-0.5, 0.5),
        vec2f(-0.5, 0.5), vec2f(0.5, -0.5), vec2f(0.5, 0.5),
    );
    let c = corners[vi];
    var l = c * size;
    let pxs = size * g.scale;
    var lc = c * 2.0;
    if (kind == 4u) {
        // Hình chữ nhật: nới ô thêm 1 px mỗi bên để mép được làm mịn (kể cả nét mảnh < 1 px)
        let e = (pxs + vec2f(2.0)) / max(pxs, vec2f(0.001));
        l = l * e;
        lc = lc * e;
    }
    let cs = cos(rot);
    let sn = sin(rot);
    let w = pos + vec2f(l.x * cs - l.y * sn, l.x * sn + l.y * cs);
    let px = w * g.scale + g.off;
    var o: VO;
    o.p = vec4f(px.x / g.res.x * 2.0 - 1.0, 1.0 - px.y / g.res.y * 2.0, 0.0, 1.0);
    o.lc = lc;
    o.pxs = pxs;
    o.kind = kind;
    o.color = color;
    o.uv = mix(uvr.xy, uvr.zw, c + 0.5);
    o.aa = 2.0 / max(min(size.x, size.y) * g.scale, 1.0);
    return o;
}

// Cùng đường cong với quầng sáng trong bản xem thử.
fn glow_profile(r: f32) -> f32 {
    if (r >= 1.0) { return 0.0; }
    if (r < 0.18) { return mix(1.0, 0.6, r / 0.18); }
    if (r < 0.5) { return mix(0.6, 0.16, (r - 0.18) / 0.32); }
    return mix(0.16, 0.0, (r - 0.5) / 0.5);
}

@fragment
fn fs(i: VO) -> @location(0) vec4f {
    if (i.kind == 3u) {
        // Ảnh (đã nhân alpha sẵn) nhân với màu phủ.
        let t = textureSampleLevel(tex, samp, i.uv, 0.0);
        return vec4f(t.rgb * i.color.rgb, t.a) * i.color.a;
    }
    if (i.kind == 4u) {
        // Độ phủ thật của điểm ảnh trên hình chữ nhật (lọc hộp theo từng trục)
        let q = abs(i.lc) * i.pxs * 0.5;
        let hw = i.pxs * 0.5;
        let cov = clamp(min(q + 0.5, hw) - max(q - 0.5, -hw), vec2f(0.0), vec2f(1.0));
        let a4 = cov.x * cov.y * i.color.a;
        return vec4f(i.color.rgb * a4, a4);
    }
    if (i.kind == 5u) {
        // Tia nắng: dải hẹp ở gốc, loe dần ra ngoài; hiện dần từ quầng mặt trời (không có mép cứng)
        // và tan dần ở ngọn; mép hai bên mờ mịn kiểu gauss.
        let t = i.lc.x * 0.5 + 0.5;
        let hw = mix(0.25, 1.0, t);
        let d = abs(i.lc.y) / hw;
        let across = exp(-d * d * 3.0);
        let along = smoothstep(0.0, 0.22, t) * pow(1.0 - t, 1.6);
        let a5 = across * along * i.color.a;
        return vec4f(i.color.rgb * a5, a5);
    }
    let r = length(i.lc);
    var a = 1.0;
    if (i.kind == 0u) {
        a = glow_profile(r);
    } else if (i.kind == 1u) {
        a = 1.0 - smoothstep(1.0 - i.aa, 1.0, r);
    }
    a *= i.color.a;
    return vec4f(i.color.rgb * a, a);
}
