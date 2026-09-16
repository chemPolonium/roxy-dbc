#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod editable_dbc;
mod export;
mod file_encoding;
mod import;
mod ui;
mod win_clipboard;

use app::AppWindow;
use std::path::PathBuf;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
};

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
    ui_state: ui::UiState,
    /// 命令行传入的待打开文件，在 resumed 中消费
    startup_files: Vec<PathBuf>,
}

/// 按扩展名打开一个 DBC / ARXML / KCD 文件，成功后记录最近文件并聚焦
fn open_path(ui_state: &mut ui::UiState, path: &std::path::Path) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let path_str = path.to_string_lossy().to_string();

    match ext.as_str() {
        "dbc" => match ui::dbc_window::DbcWindow::from_path(path) {
            Ok(dbc_window) => {
                ui_state.add_recent_file(&path_str);
                ui_state.dbc_windows.push(dbc_window);
                ui_state.last_focused_dbc_index = Some(ui_state.dbc_windows.len() - 1);
            }
            Err(e) => {
                ui_state.error_dialog.message = format!("Failed to load file: {}", e);
                ui_state.error_dialog.show = true;
            }
        },
        "arxml" | "kcd" => match crate::import::import_file(path) {
            Ok(editable_dbc) => {
                let dbc_window = ui::dbc_window::DbcWindow::new(&path_str, editable_dbc);
                ui_state.add_recent_file(&path_str);
                ui_state.dbc_windows.push(dbc_window);
                ui_state.last_focused_dbc_index = Some(ui_state.dbc_windows.len() - 1);
            }
            Err(e) => {
                ui_state.error_dialog.message = format!("Import failed: {}", e);
                ui_state.error_dialog.show = true;
            }
        },
        _ => {
            ui_state.error_dialog.message =
                format!("Unsupported file type: {} (expected .dbc / .arxml / .kcd)", path_str);
            ui_state.error_dialog.show = true;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(AppWindow::new(event_loop));
        self.ui_state.load_recent_files();

        // 打开命令行传入的文件（支持多个，按顺序打开并聚焦最后一个）
        let files = std::mem::take(&mut self.startup_files);
        for path in files {
            open_path(&mut self.ui_state, &path);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let window = self.window.as_mut().unwrap();

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
                open_path(&mut self.ui_state, &path);
                self.ui_state.file_hovering = false;
            }
            WindowEvent::HoveredFile(_path) => {
                self.ui_state.file_hovering = true;
            }
            WindowEvent::HoveredFileCancelled => {
                self.ui_state.file_hovering = false;
            }
            WindowEvent::RedrawRequested => {
                let Some(imgui) = window.imgui.as_mut() else {
                    return;
                };

                // 帧率限制：如果距离上次渲染时间太短，就跳过这次渲染
                if imgui.last_frame.elapsed() < imgui.target_frame_time {
                    return;
                }
                imgui.last_frame = Instant::now();

                if window.surface_desc.width == 0 || window.surface_desc.height == 0 {
                    return;
                }

                let frame = match window.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(e) => {
                        eprintln!("dropped frame: {e:?}");
                        return;
                    }
                };

                if let Err(e) = imgui.platform.prepare_frame(&mut imgui.context, &window.window) {
                    eprintln!("prepare_frame failed: {e}");
                    return;
                }
                let ui = imgui.context.frame();

                // 使用重构后的 UI 模块渲染界面
                ui::render_ui(ui, &mut self.ui_state);

                if let Err(e) = imgui.platform.prepare_render(ui, &window.window) {
                    eprintln!("prepare_render failed: {e}");
                }

                let mut encoder: wgpu::CommandEncoder = window
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

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

                let extent = dear_imgui_wgpu::FramebufferExtent::new(
                    window.surface_desc.width,
                    window.surface_desc.height,
                );
                let consumer = imgui.renderer.renderer_consumer().unwrap();
                let pending = imgui.context.render(consumer);
                let result = imgui.renderer.render(pending, &mut rpass, extent);

                drop(rpass);

                if let Err(e) = result {
                    eprintln!("imgui render failed: {e}");
                }

                window.queue.submit(Some(encoder.finish()));

                frame.present();
            }
            _ => (),
        }

        // 转发给 dear-imgui-winit 平台层处理输入
        if let Some(imgui) = window.imgui.as_mut() {
            if let Err(e) = imgui.platform.handle_window_event(
                &mut imgui.context,
                &window.window,
                &event,
            ) {
                eprintln!("platform handle_window_event failed: {e}");
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let window = self.window.as_mut().unwrap();
        window.window.request_redraw();
    }
}

fn main() {
    env_logger::init();

    // 命令行参数：启动时打开的文件（可多个），如 roxy-dbc.exe path\to\file.dbc
    let startup_files: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait); // 等待模式，降低CPU占用
    event_loop
        .run_app(&mut App {
            window: None,
            ui_state: ui::UiState::default(),
            startup_files,
        })
        .unwrap();
}
