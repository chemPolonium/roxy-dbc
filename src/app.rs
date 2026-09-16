//! 应用程序窗口和图形上下文管理
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{dpi::LogicalSize, event_loop::ActiveEventLoop, window::Window};

pub struct ImguiState {
    pub context: Context,
    pub platform: WinitPlatform,
    pub renderer: WgpuRenderer,
    pub clear_color: wgpu::Color,
    pub last_frame: Instant,
    pub target_frame_time: Duration, // 目标帧时间（用于限制帧率）
}

pub struct AppWindow {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub window: Arc<Window>,
    pub surface_desc: wgpu::SurfaceConfiguration,
    pub surface: wgpu::Surface<'static>,
    pub imgui: Option<ImguiState>,
}

impl AppWindow {
    pub fn setup_gpu(event_loop: &ActiveEventLoop) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let version = env!("CARGO_PKG_VERSION");

            let size = LogicalSize::new(1280.0, 720.0);

            let attributes = Window::default_attributes()
                .with_inner_size(size)
                .with_title(format!("Roxy DBC {version}"));
            Arc::new(event_loop.create_window(attributes).unwrap())
        };

        let size = window.inner_size();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        // Set up swap chain
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoNoVsync, // 最低延迟，最跟手
            desired_maximum_frame_latency: 1,             // 最小缓冲延迟
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };

        surface.configure(&device, &surface_desc);

        let imgui = None;
        Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
        }
    }

    pub fn setup_imgui(&mut self) {
        let mut context = Context::create();

        let flags = context.io().config_flags() | dear_imgui_rs::ConfigFlags::DOCKING_ENABLE;
        context.io_mut().set_config_flags(flags);
        // 仅允许拖动标题栏移动窗口，避免拖动表格等内容时误移动窗口
        context
            .io_mut()
            .set_config_windows_move_from_title_bar_only(true);

        let mut platform = WinitPlatform::new(&mut context).expect("create winit platform");
        platform
            .attach_window(
                Arc::clone(&self.window),
                HiDpiMode::Default,
                &mut context,
            )
            .expect("attach winit window");

        context
            .set_ini_filename::<std::path::PathBuf>(None)
            .expect("set ini filename");
        context.set_clipboard_backend(crate::win_clipboard::WinClipboardBackend);

        // ImGui 1.92 按平台 DPI 动态光栅化字形，字体只需给逻辑参考尺寸。
        // Inconsolata 为主字体（拉丁），CJK 回退从系统字体合并进同一图集；
        // TTC 集合取第一个 face（雅黑/宋体），失败则仅剩拉丁字形。
        const INCONSOLATA_FONT: &[u8] = include_bytes!("../fonts/Inconsolata-Regular.ttf");
        let mut sources = vec![
            // # Safety: 嵌入字体字节是完整的 TTF。
            unsafe {
                dear_imgui_rs::FontSource::ttf_data_with_size(INCONSOLATA_FONT, 13.0).with_config(
                    dear_imgui_rs::FontConfig::new()
                        .pixel_snap_h(true)
                        .oversample_h(1),
                )
            },
        ];
        for path in [
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\msyh.ttf",
            "C:\\Windows\\Fonts\\simhei.ttf",
            "C:\\Windows\\Fonts\\simsun.ttc",
        ] {
            if let Ok(bytes) = std::fs::read(path) {
                let data: &'static [u8] = Box::leak(bytes.into_boxed_slice());
                // # Safety: 字体数据与图集同生命周期，且是完整字体。
                sources.push(unsafe {
                    dear_imgui_rs::FontSource::ttf_data_with_size(data, 13.0)
                });
                break;
            }
        }
        // WGPU 渲染器为 managed-texture 模式，字集纹理由渲染器按需构建上传
        context.font_atlas().add_font(&sources);

        let clear_color = wgpu::Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        };

        let init_info = WgpuInitInfo::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_desc.format,
        );
        let renderer = WgpuRenderer::new(init_info, &mut context).expect("create wgpu renderer");

        let target_frame_time = Duration::from_secs(1) / 60; // 约60FPS，平衡响应性和性能

        self.imgui = Some(ImguiState {
            context,
            platform,
            renderer,
            clear_color,
            last_frame: Instant::now(),
            target_frame_time,
        })
    }

    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        let mut window = Self::setup_gpu(event_loop);
        window.setup_imgui();
        window
    }
}
