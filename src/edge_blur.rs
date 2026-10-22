//! A small framebuffer blur followed by a theme-colored fade at the note's edge.
use eframe::{
    egui::{self, Color32, Rect, Ui},
    glow::{self, HasContext},
};
use std::sync::{Arc, Mutex};

pub const HEIGHT: f32 = 120.0;
#[derive(Default)]
pub struct EdgeBlur {
    state: Arc<Mutex<State>>,
}
#[derive(Default)]
struct State {
    attempted: bool,
    gpu: Option<Gpu>,
}
struct Gpu {
    program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    texture: glow::NativeTexture,
    size: [i32; 2],
}
impl EdgeBlur {
    pub fn show(&self, ui: &Ui, viewport: Rect, background: Color32) {
        let fade_height = viewport.height() * 0.15;
        let rect = Rect::from_min_max(
            egui::pos2(
                viewport.left(),
                (viewport.bottom() - HEIGHT.max(fade_height)).max(viewport.top()),
            ),
            egui::pos2(viewport.right() - 14.0, viewport.bottom()),
        )
        .intersect(ui.clip_rect());
        if !rect.is_positive() {
            return;
        }
        let state = self.state.clone();
        ui.painter().add(egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, painter| {
                let Ok(mut state) = state.lock() else {
                    return;
                };
                let gl = painter.gl();
                if !state.attempted {
                    state.attempted = true;
                    // The GL context is current only inside a renderer callback.
                    state.gpu = unsafe { Gpu::new(gl) }.ok();
                }
                if let Some(gpu) = &mut state.gpu {
                    unsafe {
                        gpu.paint(gl, &info, background, fade_height);
                    }
                }
            })),
        });
        // A driver without shader support still gets a fade. On supported
        // drivers the shader masks content directly; no shaded panel is drawn.
        if !self
            .state
            .lock()
            .is_ok_and(|state| state.attempted && state.gpu.is_none())
        {
            return;
        }
        let mut mesh = egui::Mesh::default();
        for step in 0..=24 {
            let t = step as f32 / 24.0;
            let distance = rect.height() * (1.0 - t);
            let color =
                background.gamma_multiply(1.0 - (distance / fade_height.max(1.0)).clamp(0.0, 1.0));
            let y = egui::lerp(rect.top()..=rect.bottom(), t);
            for x in [rect.left(), rect.right()] {
                mesh.colored_vertex(egui::pos2(x, y), color);
            }
            if step > 0 {
                let i = (step * 2) as u32;
                mesh.add_triangle(i - 2, i - 1, i);
                mesh.add_triangle(i - 1, i + 1, i);
            }
        }
        ui.painter().add(mesh);
    }
    pub fn destroy(&self, gl: &glow::Context) {
        if let Ok(mut state) = self.state.lock()
            && let Some(gpu) = state.gpu.take()
        {
            // Called by eframe before destroying the current GL context.
            unsafe {
                gl.delete_program(gpu.program);
                gl.delete_vertex_array(gpu.vao);
                gl.delete_texture(gpu.texture);
            }
        }
    }
}
impl Gpu {
    unsafe fn new(gl: &glow::Context) -> Result<Self, String> {
        // Every created handle is released if any subsequent allocation fails.
        unsafe {
            let program = gl.create_program()?;
            for (kind, source) in [
                (glow::VERTEX_SHADER, include_str!("edge_blur.vert")),
                (glow::FRAGMENT_SHADER, include_str!("edge_blur.frag")),
            ] {
                let shader = match gl.create_shader(kind) {
                    Ok(shader) => shader,
                    Err(e) => {
                        gl.delete_program(program);
                        return Err(e);
                    }
                };
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let error = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    gl.delete_program(program);
                    return Err(error);
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                let error = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(error);
            }
            let vao = match gl.create_vertex_array() {
                Ok(vao) => vao,
                Err(e) => {
                    gl.delete_program(program);
                    return Err(e);
                }
            };
            let texture = match gl.create_texture() {
                Ok(texture) => texture,
                Err(e) => {
                    gl.delete_vertex_array(vao);
                    gl.delete_program(program);
                    return Err(e);
                }
            };
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (parameter, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, value as i32);
            }
            Ok(Self {
                program,
                vao,
                texture,
                size: [0, 0],
            })
        }
    }
    unsafe fn paint(
        &mut self,
        gl: &glow::Context,
        info: &egui::PaintCallbackInfo,
        background: Color32,
        fade_height: f32,
    ) {
        let viewport = info.viewport_in_pixels();
        if viewport.width_px <= 0 || viewport.height_px <= 0 {
            return;
        }
        let padding = (12.0 * info.pixels_per_point).ceil() as i32;
        let size = [
            viewport.width_px,
            (viewport.height_px + padding)
                .min(info.screen_size_px[1] as i32 - viewport.from_bottom_px),
        ];
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            if self.size != size {
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    size[0],
                    size[1],
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                self.size = size;
            }
            gl.copy_tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                viewport.left_px,
                viewport.from_bottom_px,
                size[0],
                size[1],
            );
            gl.disable(glow::BLEND);
            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vao));
            gl.uniform_1_i32(gl.get_uniform_location(self.program, "source").as_ref(), 0);
            gl.uniform_2_f32(
                gl.get_uniform_location(self.program, "texel").as_ref(),
                1.0 / size[0] as f32,
                1.0 / size[1] as f32,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "height_ratio")
                    .as_ref(),
                viewport.height_px as f32 / size[1] as f32,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "density").as_ref(),
                info.pixels_per_point,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "viewport_height")
                    .as_ref(),
                viewport.height_px as f32 / info.pixels_per_point,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "fade_height")
                    .as_ref(),
                fade_height.max(1.0),
            );
            gl.uniform_3_f32(
                gl.get_uniform_location(self.program, "background").as_ref(),
                background.r() as f32 / 255.0,
                background.g() as f32 / 255.0,
                background.b() as f32 / 255.0,
            );
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            // egui_glow restores its program, viewport, blend and VAO after callbacks.
            gl.bind_vertex_array(None);
        }
    }
}
