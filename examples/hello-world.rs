extern crate imgui_winit_support;

use imgui::*;
use imgui_wgpu::{Renderer, RendererConfig};
use pollster::block_on;

use std::time::Instant;
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};

fn main() {
    env_logger::init();

    // Set up window and GPU
    let event_loop = EventLoop::new().unwrap();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });

    let window = {
        let version = env!("CARGO_PKG_VERSION");

        let size = LogicalSize::new(1280.0, 720.0);
        WindowBuilder::new()
            .with_inner_size(size)
            .with_title(&format!("imgui-wgpu {version}"))
            .build(&event_loop)
            .unwrap()
    };
    let size = window.inner_size();
    let surface = instance.create_surface(&window).unwrap();

    let hidpi_factor = window.scale_factor();

    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .unwrap();

    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).unwrap();

    // Set up swap chain
    let surface_desc = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
    };

    surface.configure(&device, &surface_desc);

    // Set up dear imgui
    let mut imgui = imgui::Context::create();
    let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);
    platform.attach_window(
        imgui.io_mut(),
        &window,
        imgui_winit_support::HiDpiMode::Default,
    );
    imgui.set_ini_filename(None);

    let font_size = (13.0 * hidpi_factor) as f32;
    imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

    imgui.fonts().add_font(&[FontSource::DefaultFontData {
        config: Some(imgui::FontConfig {
            oversample_h: 1,
            pixel_snap_h: true,
            size_pixels: font_size,
            ..Default::default()
        }),
    }]);

    //
    // Set up dear imgui wgpu renderer
    //
    let clear_color = wgpu::Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };

    let renderer_config = RendererConfig {
        texture_format: surface_desc.format,
        ..Default::default()
    };

    let mut renderer = Renderer::new(&mut imgui, &device, &queue, renderer_config);

    let mut last_frame = Instant::now();
    let mut demo_open = true;

    let mut last_cursor = None;

    // Event loop
    let _ = event_loop.run(|event, elwt| {
        if cfg!(feature = "metal-auto-capture") {
            elwt.exit();
        } else {
            elwt.set_control_flow(ControlFlow::Poll);
        };
        match event {
            Event::AboutToWait => window.request_redraw(),
            Event::WindowEvent { ref event, .. } => match event {
                WindowEvent::Resized(size) => {
                    let surface_desc = wgpu::SurfaceConfiguration {
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        width: size.width,
                        height: size.height,
                        present_mode: wgpu::PresentMode::Fifo,
                        desired_maximum_frame_latency: 2,
                        alpha_mode: wgpu::CompositeAlphaMode::Auto,
                        view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
                    };

                    surface.configure(&device, &surface_desc);
                }
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::KeyboardInput { event, .. } => {
                    if let Key::Named(NamedKey::Escape) = event.logical_key {
                        if event.state.is_pressed() {
                            elwt.exit();
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    let delta_s = last_frame.elapsed();
                    let now = Instant::now();
                    imgui.io_mut().update_delta_time(now - last_frame);
                    last_frame = now;

                    let frame = match surface.get_current_texture() {
                        Ok(frame) => frame,
                        Err(e) => {
                            eprintln!("dropped frame: {e:?}");
                            return;
                        }
                    };
                    platform
                        .prepare_frame(imgui.io_mut(), &window)
                        .expect("Failed to prepare frame");
                    let ui = imgui.frame();

                    {
                        let window = ui.window("Hello world");
                        window
                            .size([300.0, 100.0], Condition::FirstUseEver)
                            .build(|| {
                                ui.text("Hello world!");
                                ui.text("This...is...imgui-rs on WGPU!");
                                ui.separator();
                                let mouse_pos = ui.io().mouse_pos;
                                ui.text(format!(
                                    "Mouse Position: ({:.1},{:.1})",
                                    mouse_pos[0], mouse_pos[1]
                                ));
                            });

                        let window = ui.window("Hello too");
                        window
                            .size([400.0, 200.0], Condition::FirstUseEver)
                            .position([400.0, 200.0], Condition::FirstUseEver)
                            .build(|| {
                                ui.text(format!("Frametime: {delta_s:?}"));
                            });

                        ui.show_demo_window(&mut demo_open);
                    }

                    let mut encoder: wgpu::CommandEncoder = device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                    if last_cursor != Some(ui.mouse_cursor()) {
                        last_cursor = Some(ui.mouse_cursor());
                        platform.prepare_render(ui, &window);
                    }

                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(clear_color),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    renderer
                        .render(imgui.render(), &queue, &device, &mut rpass)
                        .expect("Rendering failed");

                    drop(rpass);

                    queue.submit(Some(encoder.finish()));

                    frame.present();
                }
                _ => {}
            },
            _ => {}
        }

        platform.handle_event(imgui.io_mut(), &window, &event);
    });
}
