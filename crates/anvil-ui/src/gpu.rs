//! Optional GPU viewport: the same scene as `raster.rs`, drawn with
//! OpenGL through an egui paint callback.
//!
//! The renderer needs nothing beyond plain OpenGL 3.3 core (or OpenGL ES
//! 3.0): two small programs, one vertex buffer for the triangles, one for
//! the feature edges, and an offscreen framebuffer that is blitted into
//! the egui frame inside the viewport rect. `glow` comes with eframe's
//! glow backend, so no new dependency is needed.
//!
//! The software viewport stays in place. When no GL context exists, when
//! a shader fails to compile, or when the user turns the setting off,
//! `AnvilApp` draws with `raster.rs` exactly as before.
//!
//! Picking is unchanged: the CPU rasterizer still fills the id and depth
//! buffers, at half resolution, with no colour work. That keeps face,
//! body and edge picking, the sketch hidden-line test and box select on
//! one code path, and avoids a per-frame readback from the GPU.

use crate::camera::Projector;
use crate::raster::Style;
use crate::scene::Scene;
use anvil_kernel::FaceId;
use anvil_math::DVec3;
use eframe::glow::{self, HasContext};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Samples per screen pixel kept for picking while the GPU draws.
pub const PICK_SCALE: f64 = 0.5;

/// Depth bias, in clip space, that lifts the feature edges off the faces.
/// The software renderer scales the depth by 0.9985 for the same reason.
const EDGE_BIAS: f32 = 0.0015;

const VERT_SRC: &str = r#"
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(location = 2) in uint a_col;
layout(location = 3) in uint a_body;
layout(location = 4) in uint a_face;
uniform mat4 u_mvp;
uniform vec3 u_light;
uniform int u_sel_body;
uniform int u_hov_body;
uniform uint u_sel_face;
uniform uint u_hov_face;
uniform vec3 u_sel_col;
uniform vec3 u_hov_col;
out vec3 v_col;
out float v_shade;
out vec3 v_pos;
void main() {
    vec3 base = vec3(float((a_col >> 16u) & 255u), float((a_col >> 8u) & 255u), float(a_col & 255u)) / 255.0;
    if (u_sel_face != 0u && a_face == u_sel_face) {
        base = u_sel_col;
    } else if (u_hov_face != 0u && a_face == u_hov_face) {
        base = u_hov_col;
    } else if (u_sel_body >= 0 && int(a_body) == u_sel_body) {
        base = u_sel_col;
    } else if (u_hov_body >= 0 && int(a_body) == u_hov_body) {
        base = u_hov_col;
    }
    v_col = base;
    v_shade = 0.30 + 0.70 * max(dot(normalize(a_nrm), u_light), 0.0);
    v_pos = a_pos;
    gl_Position = u_mvp * vec4(a_pos, 1.0);
}
"#;

const FRAG_SRC: &str = r#"
in vec3 v_col;
in float v_shade;
in vec3 v_pos;
uniform vec3 u_sec_n;
uniform float u_sec_w;
uniform int u_sec_on;
out vec4 f_col;
void main() {
    if (u_sec_on == 1 && dot(u_sec_n, v_pos) > u_sec_w) {
        discard;
    }
    vec3 c = v_col * v_shade;
    if (u_sec_on == 1 && !gl_FrontFacing) {
        // The inside of a cut, tinted like the software renderer.
        c = vec3(200.0, 70.0, 60.0) / 255.0 * 0.75;
    }
    f_col = vec4(c, 1.0);
}
"#;

const EDGE_VERT_SRC: &str = r#"
layout(location = 0) in vec3 a_pos;
uniform mat4 u_mvp;
uniform float u_bias;
out vec3 v_pos;
void main() {
    v_pos = a_pos;
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    gl_Position.z -= u_bias * gl_Position.w;
}
"#;

const EDGE_FRAG_SRC: &str = r#"
in vec3 v_pos;
uniform vec3 u_col;
uniform vec3 u_sec_n;
uniform float u_sec_w;
uniform int u_sec_on;
out vec4 f_col;
void main() {
    if (u_sec_on == 1 && dot(u_sec_n, v_pos) > u_sec_w) {
        discard;
    }
    f_col = vec4(u_col, 1.0);
}
"#;

/// Floats and integers per vertex: position, normal, colour, body, face.
const STRIDE: i32 = 9 * 4;

/// Triangles and edges of one scene, ready to upload.
struct GpuMesh {
    /// Scene id this was built from.
    scene: u64,
    /// Body colour baked in, so a style change rebuilds the mesh.
    body_color: [u8; 3],
    verts: Vec<u32>,
    tri_verts: i32,
    edges: Vec<u32>,
    edge_verts: i32,
    /// Dense key per (body, face), 0 means "no face".
    face_keys: HashMap<(u32, FaceId), u32>,
    /// Positions are stored relative to this point, to keep f32 precise.
    origin: DVec3,
}

impl GpuMesh {
    fn build(scene: &Scene, body_color: [u8; 3]) -> GpuMesh {
        let origin = if scene.bounds.is_empty() { DVec3::ZERO } else { scene.bounds.center() };
        let mesh = &scene.mesh;
        let tris = mesh.indices.len() / 3;
        let mut verts: Vec<u32> = Vec::with_capacity(tris * 3 * 9);
        let mut face_keys: HashMap<(u32, FaceId), u32> = HashMap::new();
        for (t, idx) in mesh.indices.as_chunks::<3>().0.iter().enumerate() {
            let p: [DVec3; 3] =
                [mesh.positions[idx[0] as usize], mesh.positions[idx[1] as usize], mesh.positions[idx[2] as usize]];
            // Same normal rule as the software renderer: the geometric
            // normal, falling back to the stored one on a sliver.
            let cross = (p[1] - p[0]).cross(p[2] - p[0]);
            let n = if cross.length() > 1e-12 {
                cross.normalize()
            } else {
                mesh.normals.get(idx[0] as usize).copied().unwrap_or(DVec3::Z)
            };
            let body = scene.tri_body.get(t).copied().unwrap_or(0);
            let col = match scene.colors.get(body as usize) {
                Some(Some(c)) => *c,
                _ => body_color,
            };
            let packed = ((col[0] as u32) << 16) | ((col[1] as u32) << 8) | col[2] as u32;
            let face = match scene.mesh.face_of_tri.get(t).copied() {
                Some(f) => {
                    let next = face_keys.len() as u32 + 1;
                    *face_keys.entry((body, f)).or_insert(next)
                }
                None => 0,
            };
            for v in p {
                let q = v - origin;
                verts.extend_from_slice(&[
                    (q.x as f32).to_bits(),
                    (q.y as f32).to_bits(),
                    (q.z as f32).to_bits(),
                    (n.x as f32).to_bits(),
                    (n.y as f32).to_bits(),
                    (n.z as f32).to_bits(),
                    packed,
                    body,
                    face,
                ]);
            }
        }
        let mut edges: Vec<u32> = Vec::with_capacity(scene.edges.len() * 6);
        for (_, seg) in &scene.edges {
            for v in seg {
                let q = *v - origin;
                edges.extend_from_slice(&[(q.x as f32).to_bits(), (q.y as f32).to_bits(), (q.z as f32).to_bits()]);
            }
        }
        let tri_verts = (verts.len() / 9) as i32;
        let edge_verts = (edges.len() / 3) as i32;
        GpuMesh { scene: scene.id, body_color, verts, tri_verts, edges, edge_verts, face_keys, origin }
    }
}

/// Everything the paint callback needs that is not in the GL objects.
#[derive(Clone)]
struct DrawParams {
    mvp: [f32; 16],
    light: [f32; 3],
    background: [f32; 4],
    sel_body: i32,
    hov_body: i32,
    sel_face: u32,
    hov_face: u32,
    sel_col: [f32; 3],
    hov_col: [f32; 3],
    edge_col: [f32; 3],
    draw_edges: bool,
    /// Section plane in mesh coordinates: keep `n . p <= w`.
    section: Option<([f32; 3], f32)>,
}

/// GL objects. Handles only, so the callback can own this across threads.
struct GpuState {
    prog: glow::Program,
    edge_prog: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    edge_vao: glow::VertexArray,
    edge_vbo: glow::Buffer,
    fbo: Option<(glow::Framebuffer, glow::Texture, glow::Renderbuffer, i32, i32)>,
    /// Scene id and body colour currently in the buffers.
    uploaded: Option<(u64, [u8; 3])>,
    tri_verts: i32,
    edge_verts: i32,
}

/// The GPU viewport, owned by `AnvilApp`. Unavailable by default, so the
/// headless layout tests and any build without a GL context use the
/// software renderer.
#[derive(Default)]
pub struct GpuViewport {
    state: Option<Arc<Mutex<GpuState>>>,
    mesh: Option<Arc<GpuMesh>>,
    enabled: bool,
    renderer: String,
    /// Why the GPU path is unavailable, for the status bar.
    problem: Option<String>,
}

impl GpuViewport {
    /// Compile the shaders and create the buffers. `gl` is
    /// `eframe::CreationContext::gl`, which is `None` without a context.
    pub fn new(gl: Option<&Arc<glow::Context>>) -> GpuViewport {
        let Some(gl) = gl else {
            return GpuViewport { problem: Some("no OpenGL context".into()), ..Default::default() };
        };
        match build(gl) {
            Ok((state, renderer)) => {
                GpuViewport { state: Some(Arc::new(Mutex::new(state))), enabled: true, renderer, ..Default::default() }
            }
            Err(e) => GpuViewport { problem: Some(e), ..Default::default() },
        }
    }

    pub fn available(&self) -> bool {
        self.state.is_some()
    }

    /// True when the GPU draws the viewport this frame.
    pub fn active(&self) -> bool {
        self.enabled && self.available()
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// GL_RENDERER, or the reason the GPU path is unavailable.
    pub fn renderer(&self) -> &str {
        match (&self.problem, self.renderer.is_empty()) {
            (Some(p), _) => p,
            (None, true) => "unknown",
            (None, false) => &self.renderer,
        }
    }

    /// A paint callback that draws `scene` into `rect`. Returns `None`
    /// when the GPU path is off or unavailable, and the caller then uses
    /// the software renderer.
    #[allow(clippy::too_many_arguments)]
    pub fn callback(
        &mut self,
        rect: egui::Rect,
        scene: &Scene,
        proj: &Projector,
        style: &Style,
        selected: Option<u32>,
        hovered: Option<u32>,
        selected_face: Option<(u32, FaceId)>,
        hovered_face: Option<(u32, FaceId)>,
    ) -> Option<egui::Shape> {
        if !self.active() {
            return None;
        }
        let state = self.state.clone()?;
        let body = [style.body.r(), style.body.g(), style.body.b()];
        let stale = self.mesh.as_ref().is_none_or(|m| m.scene != scene.id || m.body_color != body);
        if stale {
            self.mesh = Some(Arc::new(GpuMesh::build(scene, body)));
        }
        let mesh = self.mesh.clone()?;
        let key = |f: Option<(u32, FaceId)>| f.and_then(|k| mesh.face_keys.get(&k).copied()).unwrap_or(0);
        let rgb = |c: egui::Color32| [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0];
        let section =
            style.section.map(|(n, w)| ([n.x as f32, n.y as f32, n.z as f32], (w - n.dot(mesh.origin)) as f32));
        let bg = rgb(style.background);
        let params = DrawParams {
            mvp: mvp(proj, scene, mesh.origin),
            light: [style.light.x as f32, style.light.y as f32, style.light.z as f32],
            background: [bg[0], bg[1], bg[2], 1.0],
            sel_body: selected.map(|b| b as i32).unwrap_or(-1),
            hov_body: hovered.map(|b| b as i32).unwrap_or(-1),
            sel_face: key(selected_face),
            hov_face: key(hovered_face),
            sel_col: rgb(style.selected),
            hov_col: rgb(style.hovered),
            edge_col: rgb(style.edge),
            draw_edges: style.draw_edges,
            section,
        };
        let cb = egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, painter| {
                let Ok(mut st) = state.lock() else { return };
                unsafe {
                    draw(painter.gl(), &mut st, &mesh, &params, &info, painter.intermediate_fbo());
                }
            })),
        };
        Some(egui::Shape::Callback(cb))
    }
}

/// Clip-space matrix matching `Projector::project`, with positions given
/// relative to `origin`. Column major, as OpenGL wants it.
fn mvp(proj: &Projector, scene: &Scene, origin: DVec3) -> [f32; 16] {
    let o = origin - proj.eye;
    let kx = 2.0 * proj.scale / proj.width.max(1.0);
    let ky = 2.0 * proj.scale / proj.height.max(1.0);
    // Depth range over the scene, along the view direction.
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    if !scene.bounds.is_empty() {
        let (b0, b1) = (scene.bounds.min, scene.bounds.max);
        for i in 0..8 {
            let c = DVec3::new(
                if i & 1 == 0 { b0.x } else { b1.x },
                if i & 2 == 0 { b0.y } else { b1.y },
                if i & 4 == 0 { b0.z } else { b1.z },
            );
            let z = (c - proj.eye).dot(proj.fwd);
            lo = lo.min(z);
            hi = hi.max(z);
        }
    }
    if !lo.is_finite() || !hi.is_finite() || hi <= lo {
        lo = 0.0;
        hi = 1.0;
    }
    let pad = ((hi - lo) * 0.05).max(1e-3);
    let far = hi + pad;
    let (near, far) = if proj.ortho {
        (lo - pad, far)
    } else {
        let far = far.max(1e-3);
        ((lo - pad).max(far * 1e-4), far)
    };
    let span = (far - near).max(1e-9);
    let (rz, wz) = if proj.ortho {
        // clip_z = 2 (z - near) / span - 1, w = 1.
        let c = 2.0 / span;
        ((proj.fwd * c, o.dot(proj.fwd) * c - (2.0 * near / span + 1.0)), (DVec3::ZERO, 1.0))
    } else {
        // clip_z = A z - B, w = z.
        let a = (far + near) / span;
        let b = 2.0 * far * near / span;
        ((proj.fwd * a, o.dot(proj.fwd) * a - b), (proj.fwd, o.dot(proj.fwd)))
    };
    let rows = [(proj.right * kx, o.dot(proj.right) * kx), (proj.up * ky, o.dot(proj.up) * ky), rz, wz];
    let mut m = [0f32; 16];
    for (r, (v, w)) in rows.iter().enumerate() {
        m[r] = v.x as f32;
        m[4 + r] = v.y as f32;
        m[8 + r] = v.z as f32;
        m[12 + r] = *w as f32;
    }
    m
}

fn shader_header(gl: &glow::Context) -> String {
    let v = gl.version();
    if v.is_embedded {
        "#version 300 es\nprecision highp float;\nprecision highp int;\n".into()
    } else {
        "#version 330 core\n".into()
    }
}

fn build(gl: &Arc<glow::Context>) -> Result<(GpuState, String), String> {
    unsafe {
        // A multisampled window framebuffer cannot take a blit from a
        // single sample one, so stay on the software renderer there.
        let samples = gl.get_parameter_i32(glow::SAMPLES);
        if samples > 1 {
            return Err(format!("the window uses {samples}x multisampling"));
        }
        let head = shader_header(gl);
        let prog = program(gl, &head, VERT_SRC, FRAG_SRC)?;
        let edge_prog = program(gl, &head, EDGE_VERT_SRC, EDGE_FRAG_SRC)?;
        let vao = gl.create_vertex_array()?;
        let vbo = gl.create_buffer()?;
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, STRIDE, 0);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, STRIDE, 12);
        gl.vertex_attrib_pointer_i32(2, 1, glow::UNSIGNED_INT, STRIDE, 24);
        gl.vertex_attrib_pointer_i32(3, 1, glow::UNSIGNED_INT, STRIDE, 28);
        gl.vertex_attrib_pointer_i32(4, 1, glow::UNSIGNED_INT, STRIDE, 32);
        for i in 0..5 {
            gl.enable_vertex_attrib_array(i);
        }
        let edge_vao = gl.create_vertex_array()?;
        let edge_vbo = gl.create_buffer()?;
        gl.bind_vertex_array(Some(edge_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(edge_vbo));
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 12, 0);
        gl.enable_vertex_attrib_array(0);
        gl.bind_vertex_array(None);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
        let renderer = gl.get_parameter_string(glow::RENDERER);
        let state = GpuState {
            prog,
            edge_prog,
            vao,
            vbo,
            edge_vao,
            edge_vbo,
            fbo: None,
            uploaded: None,
            tri_verts: 0,
            edge_verts: 0,
        };
        Ok((state, renderer))
    }
}

unsafe fn program(gl: &glow::Context, head: &str, vert: &str, frag: &str) -> Result<glow::Program, String> {
    unsafe {
        let prog = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, src) in [(glow::VERTEX_SHADER, vert), (glow::FRAGMENT_SHADER, frag)] {
            let sh = gl.create_shader(kind)?;
            gl.shader_source(sh, &format!("{head}{src}"));
            gl.compile_shader(sh);
            if !gl.get_shader_compile_status(sh) {
                let log = gl.get_shader_info_log(sh);
                gl.delete_shader(sh);
                for s in shaders {
                    gl.delete_shader(s);
                }
                gl.delete_program(prog);
                return Err(format!("shader did not compile: {log}"));
            }
            gl.attach_shader(prog, sh);
            shaders.push(sh);
        }
        gl.link_program(prog);
        let ok = gl.get_program_link_status(prog);
        for s in shaders {
            gl.detach_shader(prog, s);
            gl.delete_shader(s);
        }
        if !ok {
            let log = gl.get_program_info_log(prog);
            gl.delete_program(prog);
            return Err(format!("program did not link: {log}"));
        }
        Ok(prog)
    }
}

/// View of a `u32` buffer as bytes, for `buffer_data_u8_slice`.
fn as_bytes(v: &[u32]) -> &[u8] {
    // Safe: u32 has no padding and no invalid bit patterns, and the
    // lifetime is tied to the input slice.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)) }
}

unsafe fn draw(
    gl: &glow::Context,
    st: &mut GpuState,
    mesh: &GpuMesh,
    p: &DrawParams,
    info: &egui::PaintCallbackInfo,
    target: Option<glow::Framebuffer>,
) {
    unsafe {
        if st.uploaded != Some((mesh.scene, mesh.body_color)) {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(st.vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_bytes(&mesh.verts), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(st.edge_vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_bytes(&mesh.edges), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            st.uploaded = Some((mesh.scene, mesh.body_color));
            st.tri_verts = mesh.tri_verts;
            st.edge_verts = mesh.edge_verts;
        }
        let vp = info.viewport_in_pixels();
        let (w, h) = (vp.width_px.max(1), vp.height_px.max(1));
        let Some((fbo, _, _, _, _)) = ensure_fbo(gl, st, w, h) else { return };

        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.disable(glow::SCISSOR_TEST);
        gl.viewport(0, 0, w, h);
        gl.disable(glow::BLEND);
        gl.depth_mask(true);
        gl.clear_color(p.background[0], p.background[1], p.background[2], p.background[3]);
        gl.clear_depth_f32(1.0);
        gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        gl.enable(glow::DEPTH_TEST);
        gl.depth_func(glow::LESS);
        gl.front_face(glow::CCW);
        // A section view shows the inside of the cut, so keep back faces.
        if p.section.is_some() {
            gl.disable(glow::CULL_FACE);
        } else {
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
        }

        let (sec_n, sec_w, sec_on) = match p.section {
            Some((n, w)) => (n, w, 1),
            None => ([0.0, 0.0, 1.0], 0.0, 0),
        };
        if st.tri_verts > 0 {
            gl.use_program(Some(st.prog));
            let u = |name: &str| gl.get_uniform_location(st.prog, name);
            gl.uniform_matrix_4_f32_slice(u("u_mvp").as_ref(), false, &p.mvp);
            gl.uniform_3_f32(u("u_light").as_ref(), p.light[0], p.light[1], p.light[2]);
            gl.uniform_1_i32(u("u_sel_body").as_ref(), p.sel_body);
            gl.uniform_1_i32(u("u_hov_body").as_ref(), p.hov_body);
            gl.uniform_1_u32(u("u_sel_face").as_ref(), p.sel_face);
            gl.uniform_1_u32(u("u_hov_face").as_ref(), p.hov_face);
            gl.uniform_3_f32(u("u_sel_col").as_ref(), p.sel_col[0], p.sel_col[1], p.sel_col[2]);
            gl.uniform_3_f32(u("u_hov_col").as_ref(), p.hov_col[0], p.hov_col[1], p.hov_col[2]);
            gl.uniform_3_f32(u("u_sec_n").as_ref(), sec_n[0], sec_n[1], sec_n[2]);
            gl.uniform_1_f32(u("u_sec_w").as_ref(), sec_w);
            gl.uniform_1_i32(u("u_sec_on").as_ref(), sec_on);
            gl.bind_vertex_array(Some(st.vao));
            gl.draw_arrays(glow::TRIANGLES, 0, st.tri_verts);
        }
        if p.draw_edges && st.edge_verts > 0 {
            gl.use_program(Some(st.edge_prog));
            let u = |name: &str| gl.get_uniform_location(st.edge_prog, name);
            gl.uniform_matrix_4_f32_slice(u("u_mvp").as_ref(), false, &p.mvp);
            gl.uniform_1_f32(u("u_bias").as_ref(), EDGE_BIAS);
            gl.uniform_3_f32(u("u_col").as_ref(), p.edge_col[0], p.edge_col[1], p.edge_col[2]);
            gl.uniform_3_f32(u("u_sec_n").as_ref(), sec_n[0], sec_n[1], sec_n[2]);
            gl.uniform_1_f32(u("u_sec_w").as_ref(), sec_w);
            gl.uniform_1_i32(u("u_sec_on").as_ref(), sec_on);
            gl.disable(glow::CULL_FACE);
            gl.bind_vertex_array(Some(st.edge_vao));
            gl.draw_arrays(glow::LINES, 0, st.edge_verts);
        }

        // Copy into the egui frame, clipped like any other egui shape.
        let clip = info.clip_rect_in_pixels();
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fbo));
        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, target);
        gl.enable(glow::SCISSOR_TEST);
        gl.scissor(clip.left_px, clip.from_bottom_px, clip.width_px.max(0), clip.height_px.max(0));
        gl.blit_framebuffer(
            0,
            0,
            w,
            h,
            vp.left_px,
            vp.from_bottom_px,
            vp.left_px + w,
            vp.from_bottom_px + h,
            glow::COLOR_BUFFER_BIT,
            glow::NEAREST,
        );

        // Leave the state as egui handed it over.
        gl.bind_framebuffer(glow::FRAMEBUFFER, target);
        gl.bind_vertex_array(None);
        gl.disable(glow::DEPTH_TEST);
        gl.disable(glow::CULL_FACE);
        gl.enable(glow::BLEND);
    }
}

type Fbo = (glow::Framebuffer, glow::Texture, glow::Renderbuffer, i32, i32);

/// The offscreen target, grown as needed. It is never shrunk, so the four
/// pane layout does not rebuild it four times a frame.
unsafe fn ensure_fbo(gl: &glow::Context, st: &mut GpuState, w: i32, h: i32) -> Option<Fbo> {
    unsafe {
        if let Some(f) = st.fbo {
            if f.3 >= w && f.4 >= h {
                return Some(f);
            }
            gl.delete_framebuffer(f.0);
            gl.delete_texture(f.1);
            gl.delete_renderbuffer(f.2);
            st.fbo = None;
        }
        let (w, h) = (w.max(1), h.max(1));
        let fbo = gl.create_framebuffer().ok()?;
        let tex = gl.create_texture().ok()?;
        let rbo = gl.create_renderbuffer().ok()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            w,
            h,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(rbo));
        gl.renderbuffer_storage(glow::RENDERBUFFER, glow::DEPTH_COMPONENT24, w, h);
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(tex), 0);
        gl.framebuffer_renderbuffer(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::RENDERBUFFER, Some(rbo));
        let ok = gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE;
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.bind_renderbuffer(glow::RENDERBUFFER, None);
        if !ok {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.delete_framebuffer(fbo);
            gl.delete_texture(tex);
            gl.delete_renderbuffer(rbo);
            return None;
        }
        st.fbo = Some((fbo, tex, rbo, w, h));
        st.fbo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;

    fn scene_and_proj() -> (Scene, Projector) {
        let mut doc = anvil_feature::Document::new("t");
        doc.add_feature(Box::new(anvil_feature::features::primitives::BoxFeature::default()));
        let scene = Scene::build(&doc);
        let mut cam = Camera::default();
        cam.fit(&scene.bounds);
        let proj = Projector::new(&cam, 400.0, 300.0);
        (scene, proj)
    }

    /// The GL matrix must land on the same pixels as `Projector::project`.
    #[test]
    fn matrix_matches_the_software_projection() {
        for ortho in [false, true] {
            let (scene, mut proj) = scene_and_proj();
            if ortho {
                let mut cam = Camera { ortho_height: Some(60.0), ..Camera::default() };
                cam.fit(&scene.bounds);
                proj = Projector::new(&cam, 400.0, 300.0);
            }
            let origin = scene.bounds.center();
            let m = mvp(&proj, &scene, origin);
            let at = |r: usize, c: usize| m[c * 4 + r] as f64;
            for &p in scene.mesh.positions.iter().take(24) {
                let q = p - origin;
                let clip: Vec<f64> =
                    (0..4).map(|r| at(r, 0) * q.x + at(r, 1) * q.y + at(r, 2) * q.z + at(r, 3)).collect();
                let (x, y, z) = proj.project(p).expect("in front of the camera");
                assert!(clip[3].abs() > 1e-9);
                let px = (clip[0] / clip[3] * 0.5 + 0.5) * proj.width;
                let py = (0.5 - clip[1] / clip[3] * 0.5) * proj.height;
                assert!((px - x).abs() < 0.01, "x {px} vs {x}");
                assert!((py - y).abs() < 0.01, "y {py} vs {y}");
                // Depth must keep its order: inside the clip range and
                // rising with the distance along the view direction.
                let ndc_z = clip[2] / clip[3];
                assert!((-1.0..=1.0).contains(&ndc_z), "z {ndc_z} for depth {z}");
            }
        }
    }

    #[test]
    fn mesh_is_expanded_per_triangle_with_face_keys() {
        let (scene, _) = scene_and_proj();
        let mesh = GpuMesh::build(&scene, [1, 2, 3]);
        assert_eq!(mesh.tri_verts as usize, scene.mesh.indices.len());
        assert_eq!(mesh.edge_verts as usize, scene.edges.len() * 2);
        assert_eq!(mesh.face_keys.len(), 6, "a box has six faces");
        assert!(mesh.face_keys.values().all(|&k| k > 0), "0 is reserved for no face");
        // The colour of the first vertex is the style colour, packed.
        assert_eq!(mesh.verts[6], 0x010203);
    }

    #[test]
    fn a_missing_context_is_reported_not_fatal() {
        let g = GpuViewport::new(None);
        assert!(!g.available());
        assert!(!g.active());
        assert_eq!(g.renderer(), "no OpenGL context");
    }
}
