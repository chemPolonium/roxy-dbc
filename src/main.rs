mod app;
mod editable_dbc;
mod export;
mod import;
mod ui;

use app::AppWindow;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
};

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
    ui_state: ui::UiState,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(AppWindow::new(event_loop));
        self.ui_state.load_recent_files();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();

        match &event {
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    return;
                }

                window.surface_desc = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    width: size.width,
                    height: size.height,
                    present_mode: wgpu::PresentMode::AutoNoVsync, // 最低延迟，最跟手
                    desired_maximum_frame_latency: 1,             // 最小缓冲延迟
                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
                    view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
                };

                window
                    .surface
                    .configure(&window.device, &window.surface_desc);
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::DroppedFile(path) => {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                let path_str = path.to_string_lossy().to_string();

                match ext.as_str() {
                    "dbc" => {
                        match crate::ui::dbc_window::DbcWindow::from_path(path) {
                            Ok(dbc_window) => {
                                self.ui_state.add_recent_file(&path_str);
                                self.ui_state.dbc_windows.push(dbc_window);
                                self.ui_state.last_focused_dbc_index =
                                    Some(self.ui_state.dbc_windows.len() - 1);
                            }
                            Err(e) => {
                                self.ui_state.error_dialog.message =
                                    format!("Failed to load dropped file: {}", e);
                                self.ui_state.error_dialog.show = true;
                            }
                        }
                    }
                    "arxml" | "kcd" => {
                        match crate::import::import_file(path) {
                            Ok(editable_dbc) => {
                                let dbc_window =
                                    crate::ui::dbc_window::DbcWindow::new(&path_str, editable_dbc);
                                self.ui_state.add_recent_file(&path_str);
                                self.ui_state.dbc_windows.push(dbc_window);
                                self.ui_state.last_focused_dbc_index =
                                    Some(self.ui_state.dbc_windows.len() - 1);
                            }
                            Err(e) => {
                                self.ui_state.error_dialog.message =
                                    format!("Import failed: {}", e);
                                self.ui_state.error_dialog.show = true;
                            }
                        }
                    }
                    _ => {}
                }
                self.ui_state.file_hovering = false;
            }
            WindowEvent::HoveredFile(_path) => {
                self.ui_state.file_hovering = true;
            }
            WindowEvent::HoveredFileCancelled => {
                self.ui_state.file_hovering = false;
            }
            // WindowEvent::KeyboardInput { event, .. } => {
            //     if let Key::Named(NamedKey::Escape) = event.logical_key {
            //         if event.state.is_pressed() {
            //             event_loop.exit();
            //         }
            //     }
            // }
            WindowEvent::RedrawRequested => {
                let delta_s = imgui.last_frame.elapsed();

                // 帧率限制：如果距离上次渲染时间太短，就跳过这次渲染
                if delta_s < imgui.target_frame_time {
                    return;
                }

                if window.surface_desc.width == 0 || window.surface_desc.height == 0 {
                    return;
                }

                let now = Instant::now();
                imgui
                    .context
                    .io_mut()
                    .update_delta_time(now - imgui.last_frame);
                imgui.last_frame = now;

                let frame = match window.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(e) => {
                        eprintln!("dropped frame: {e:?}");
                        return;
                    }
                };
                imgui
                    .platform
                    .prepare_frame(imgui.context.io_mut(), &window.window)
                    .expect("Failed to prepare frame");
                let ui = imgui.context.frame();

                // 使用重构后的 UI 模块渲染界面
                ui::render_ui(&ui, delta_s, imgui.target_frame_time, &mut self.ui_state);

                let mut encoder: wgpu::CommandEncoder = window
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                if imgui.last_cursor != ui.mouse_cursor() {
                    imgui.last_cursor = ui.mouse_cursor();
                    imgui.platform.prepare_render(ui, &window.window);
                }

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(imgui.clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

                imgui
                    .renderer
                    .render(
                        imgui.context.render(),
                        &window.queue,
                        &window.device,
                        &mut rpass,
                    )
                    .expect("Rendering failed");

                drop(rpass);

                window.queue.submit(Some(encoder.finish()));

                frame.present();
            }
            _ => (),
        }

        imgui.platform.handle_event::<()>(
            imgui.context.io_mut(),
            &window.window,
            &Event::WindowEvent { window_id, event },
        );
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: ()) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();
        imgui.platform.handle_event::<()>(
            imgui.context.io_mut(),
            &window.window,
            &Event::UserEvent(event),
        );
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();
        imgui.platform.handle_event::<()>(
            imgui.context.io_mut(),
            &window.window,
            &Event::DeviceEvent { device_id, event },
        );
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();
        window.window.request_redraw();
        imgui.platform.handle_event::<()>(
            imgui.context.io_mut(),
            &window.window,
            &Event::AboutToWait,
        );
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait); // 等待模式，降低CPU占用
    event_loop.run_app(&mut App::default()).unwrap();
}
