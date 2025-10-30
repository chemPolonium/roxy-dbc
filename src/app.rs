//! 应用程序窗口和图形上下文管理
use imgui::*;
use imgui_wgpu::{Renderer, RendererConfig};
use imgui_winit_support::WinitPlatform;
use pollster::block_on;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{dpi::LogicalSize, event_loop::ActiveEventLoop, window::Window};

pub struct ImguiState {
    pub context: imgui::Context,
    pub platform: WinitPlatform,
    pub renderer: Renderer,
    pub clear_color: wgpu::Color,
    pub last_frame: Instant,
    pub last_cursor: Option<MouseCursor>,
    pub target_frame_time: Duration, // 目标帧时间（用于限制帧率）
}

pub struct AppWindow {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub window: Arc<Window>,
    pub surface_desc: wgpu::SurfaceConfiguration,
    pub surface: wgpu::Surface<'static>,
    pub hidpi_factor: f64,
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
        let hidpi_factor = window.scale_factor();
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
            hidpi_factor,
            imgui,
        }
    }

    pub fn setup_imgui(&mut self) {
        let mut context = imgui::Context::create();

        context.io_mut().config_flags |= ConfigFlags::DOCKING_ENABLE;

        let mut platform = imgui_winit_support::WinitPlatform::new(&mut context);
        platform.attach_window(
            context.io_mut(),
            &self.window,
            imgui_winit_support::HiDpiMode::Default,
        );
        context.set_ini_filename(None);

        let font_size = (13.0 * self.hidpi_factor) as f32;
        context.io_mut().font_global_scale = (1.0 / self.hidpi_factor) as f32;

        // 加载嵌入的Inconsolata字体
        let font_config = imgui::FontConfig {
            oversample_h: 1,
            pixel_snap_h: true,
            size_pixels: font_size,
            ..Default::default()
        };

        let mut font_loaded = false;

        // 尝试使用嵌入的字体数据
        const INCONSOLATA_FONT: &[u8] = include_bytes!("../fonts/Inconsolata-Regular.ttf");

        if !INCONSOLATA_FONT.is_empty() {
            context.fonts().add_font(&[FontSource::TtfData {
                data: INCONSOLATA_FONT,
                size_pixels: font_size,
                config: Some(font_config.clone()),
            }]);
            font_loaded = true;
            log::info!(
                "Successfully loaded embedded Inconsolata font ({} bytes)",
                INCONSOLATA_FONT.len()
            );
        }

        // 如果嵌入字体加载失败，使用默认字体
        if !font_loaded {
            context.fonts().add_font(&[FontSource::DefaultFontData {
                config: Some(font_config),
            }]);
            log::info!("Using default font (embedded Inconsolata font not available)");
        }

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
            texture_format: self.surface_desc.format,
            ..Default::default()
        };

        let renderer = Renderer::new(&mut context, &self.device, &self.queue, renderer_config);
        let last_frame = Instant::now();
        let last_cursor = None;
        let target_frame_time = Duration::from_secs(1) / 60; // 约60FPS，平衡响应性和性能

        self.imgui = Some(ImguiState {
            context,
            platform,
            renderer,
            clear_color,
            last_frame,
            last_cursor,
            target_frame_time,
        })
    }

    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        let mut window = Self::setup_gpu(event_loop);
        window.setup_imgui();
        window
    }
}
