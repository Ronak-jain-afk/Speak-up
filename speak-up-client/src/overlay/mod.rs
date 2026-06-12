use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use egui::{Color32, Frame, Rounding, Stroke};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowLevel},
};

#[derive(Clone)]
pub struct OverlayConfig {
    pub position: OverlayPosition,
    pub opacity: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            position: OverlayPosition::BottomRight,
            opacity: 0.85,
            width: 400.0,
            height: 120.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum OverlayPosition {
    NearCursor,
    BottomRight,
    TopRight,
}

#[derive(Clone)]
pub struct OverlayState {
    pub is_visible: bool,
    pub is_recording: bool,
    pub is_processing: bool,
    pub audio_level: f32,
    pub transcript: String,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            is_visible: false,
            is_recording: false,
            is_processing: false,
            audio_level: 0.0,
            transcript: String::new(),
        }
    }
}

struct WgpuCtx {
    window: Window,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    egui_ctx: egui::Context,
}

pub fn run_overlay_loop(config: OverlayConfig, state_rx: Receiver<OverlayState>) {
    while let Ok(mut live_state) = state_rx.recv() {
        if !live_state.is_visible {
            continue;
        }

        let rx = state_rx.clone();
        let cfg = config.clone();

        let Ok(event_loop) = EventLoop::new() else {
            tracing::error!("Failed to create event loop");
            continue;
        };

        let ctx_shared: Arc<Mutex<Option<WgpuCtx>>> = Arc::new(Mutex::new(None));
        let ctx_shared2 = ctx_shared.clone();

        let attrs = Window::default_attributes()
            .with_title("Speak Up")
            .with_decorations(false)
            .with_transparent(true)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(config.width, config.height));

        let target_frame_time = Duration::from_secs_f32(1.0 / 30.0);

        #[allow(deprecated)]
        let _ = event_loop.run(move |event, target| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    target.exit();
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } if size.width > 0 && size.height > 0 => {
                    if let Ok(mut guard) = ctx_shared2.lock() {
                        if let Some(ref mut ctx) = *guard {
                            ctx.surface_config.width = size.width;
                            ctx.surface_config.height = size.height;
                            ctx.surface.configure(
                                &ctx.device,
                                &ctx.surface_config.clone(),
                            );
                        }
                    }
                }
                Event::AboutToWait => {
                    if let Ok(update) = rx.try_recv() {
                        if !update.is_visible {
                            target.exit();
                            return;
                        }
                        live_state = update;
                    }

                    let mut guard = match ctx_shared2.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };

                    if guard.is_none() {
                        let window = match target.create_window(attrs.clone())
                        {
                            Ok(w) => w,
                            Err(e) => {
                                tracing::error!(
                                    "Failed to create window: {}",
                                    e
                                );
                                target.exit();
                                return;
                            }
                        };

                        let wgpu_instance = wgpu::Instance::new(
                            wgpu::InstanceDescriptor {
                                backends: wgpu::Backends::all(),
                                ..Default::default()
                            },
                        );

                        let surface =
                            wgpu_instance.create_surface(&window).unwrap();
                        let surface: wgpu::Surface<'static> = unsafe {
                            std::mem::transmute(surface)
                        };

                        let adapter = {
                            let adapter = pollster::block_on(
                                wgpu_instance.request_adapter(
                                    &wgpu::RequestAdapterOptions {
                                        power_preference:
                                            wgpu::PowerPreference::HighPerformance,
                                        compatible_surface: Some(&surface),
                                        force_fallback_adapter: false,
                                    },
                                ),
                            );
                            match adapter {
                                Some(a) => a,
                                None => {
                                    tracing::error!(
                                        "No compatible GPU adapter"
                                    );
                                    target.exit();
                                    return;
                                }
                            }
                        };

                        let (device, queue) = match pollster::block_on(
                            adapter.request_device(
                                &wgpu::DeviceDescriptor {
                                    label: Some("overlay device"),
                                    required_features:
                                        wgpu::Features::empty(),
                                    required_limits:
                                        wgpu::Limits::default(),
                                    memory_hints:
                                        wgpu::MemoryHints::Performance,
                                },
                                None,
                            ),
                        ) {
                            Ok(d) => d,
                            Err(e) => {
                                tracing::error!(
                                    "Failed to create device: {}",
                                    e
                                );
                                target.exit();
                                return;
                            }
                        };

                        let surface_caps =
                            surface.get_capabilities(&adapter);
                        let surface_format = surface_caps
                            .formats
                            .iter()
                            .find(|f| f.is_srgb())
                            .copied()
                            .unwrap_or(surface_caps.formats[0]);

                        let size = window.inner_size();
                        let surface_config =
                            wgpu::SurfaceConfiguration {
                                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                                format: surface_format,
                                width: size.width,
                                height: size.height,
                                present_mode:
                                    wgpu::PresentMode::AutoNoVsync,
                                alpha_mode:
                                    wgpu::CompositeAlphaMode::Auto,
                                view_formats: vec![],
                                desired_maximum_frame_latency: 2,
                            };
                        surface.configure(&device, &surface_config);

                        let egui_ctx = egui::Context::default();
                        let egui_state = egui_winit::State::new(
                            egui_ctx.clone(),
                            egui::ViewportId::ROOT,
                            target,
                            None,
                            None,
                            None,
                        );

                        let egui_renderer = egui_wgpu::Renderer::new(
                            &device,
                            surface_format,
                            None,
                            1,
                            false,
                        );

                        *guard = Some(WgpuCtx {
                            window,
                            surface,
                            device,
                            queue,
                            surface_config,
                            egui_state,
                            egui_renderer,
                            egui_ctx,
                        });
                    }

                    let ctx = guard.as_mut().unwrap();

                    let frame = match ctx.surface.get_current_texture() {
                        Ok(f) => f,
                        Err(wgpu::SurfaceError::Lost) => {
                            let size = ctx.window.inner_size();
                            ctx.surface_config.width = size.width;
                            ctx.surface_config.height = size.height;
                            ctx.surface.configure(
                                &ctx.device,
                                &ctx.surface_config.clone(),
                            );
                            return;
                        }
                        Err(e) => {
                            tracing::warn!("Surface error: {:?}", e);
                            return;
                        }
                    };

                    let view = frame.texture.create_view(
                        &wgpu::TextureViewDescriptor::default(),
                    );

                    let egui_input =
                        ctx.egui_state.take_egui_input(&ctx.window);

                    let full_output = ctx.egui_ctx.run(egui_input, |ui_ctx| {
                        render_overlay_ui(ui_ctx, &live_state, &cfg);
                    });

                    let screen_descriptor =
                        egui_wgpu::ScreenDescriptor {
                            size_in_pixels: [
                                ctx.window.inner_size().width,
                                ctx.window.inner_size().height,
                            ],
                            pixels_per_point: ctx.window.scale_factor()
                                as f32,
                        };

                    let shapes = ctx.egui_ctx.tessellate(
                        full_output.shapes,
                        full_output.pixels_per_point,
                    );

                    for (id, delta) in &full_output.textures_delta.set {
                        ctx.egui_renderer.update_texture(
                            &ctx.device,
                            &ctx.queue,
                            *id,
                            delta,
                        );
                    }

                    let mut encoder =
                        ctx.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("overlay encoder"),
                            },
                        );

                    {
                        let pass = encoder
                            .begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some("overlay pass"),
                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: &view,
                                            resolve_target: None,
                                            ops: wgpu::Operations {
                                                load: wgpu::LoadOp::Clear(
                                                    wgpu::Color {
                                                        r: 0.0,
                                                        g: 0.0,
                                                        b: 0.0,
                                                        a: 0.0,
                                                    },
                                                ),
                                                store: wgpu::StoreOp::Store,
                                            },
                                        },
                                    )],
                                    depth_stencil_attachment: None,
                                    occlusion_query_set: None,
                                    timestamp_writes: None,
                                },
                            );

                        ctx.egui_renderer.render(
                            &mut pass.forget_lifetime(),
                            &shapes,
                            &screen_descriptor,
                        );
                    }

                    ctx.queue.submit(Some(encoder.finish()));
                    frame.present();

                    let elapsed = Instant::now().elapsed();
                    if elapsed < target_frame_time {
                        std::thread::sleep(
                            target_frame_time - elapsed,
                        );
                    }
                }
                _ => {}
            }
        });
    }
}

fn render_overlay_ui(ctx: &egui::Context, state: &OverlayState, config: &OverlayConfig) {
    let window_size = egui::vec2(config.width, config.height);

    egui::Area::new(egui::Id::new("overlay"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .interactable(false)
        .show(ctx, |ui| {
            let backdrop = Frame {
                fill: Color32::from_black_alpha((config.opacity * 255.0) as u8),
                rounding: Rounding::same(8.0),
                ..Default::default()
            };
            backdrop.show(ui, |ui| {
                ui.set_min_size(window_size);
                ui.set_max_size(window_size);

                ui.horizontal(|ui| {
                    let mic_color = if state.is_recording {
                        Color32::RED
                    } else {
                        Color32::GRAY
                    };
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(14.0, 14.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().circle(
                        rect.center(),
                        6.0,
                        mic_color,
                        Stroke::new(1.0, Color32::WHITE),
                    );

                    if state.is_recording {
                        ui.label(
                            egui::RichText::new("REC").color(Color32::RED).size(11.0),
                        );
                    }

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let meter_width = 80.0;
                            let meter_height = 6.0;
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(meter_width, meter_height),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect(
                                rect,
                                Rounding::same(3.0),
                                Color32::from_gray(40),
                                Stroke::new(0.0, Color32::TRANSPARENT),
                            );
                            let fill_width = (state.audio_level * meter_width)
                                .clamp(0.0, meter_width);
                            if fill_width > 0.0 {
                                let fill_rect = egui::Rect::from_min_size(
                                    rect.min,
                                    egui::vec2(fill_width, meter_height),
                                );
                                let level_color = if state.is_recording {
                                    Color32::from_rgb(0, 200, 80)
                                } else {
                                    Color32::GRAY
                                };
                                ui.painter().rect(
                                    fill_rect,
                                    Rounding::same(3.0),
                                    level_color,
                                    Stroke::new(0.0, Color32::TRANSPARENT),
                                );
                            }
                        },
                    );
                });

                ui.add_space(4.0);

                if state.is_recording {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&state.transcript)
                                .color(Color32::WHITE)
                                .size(13.0),
                        )
                        .wrap(),
                    );
                } else if state.is_processing {
                    ui.label(
                        egui::RichText::new("cleaning...")
                            .color(Color32::LIGHT_GRAY)
                            .size(11.0)
                            .italics(),
                    );
                }
            });
        });
}
