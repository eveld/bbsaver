mod ansi;
mod atlas;
mod cell;
mod cp437;
mod pack;
mod renderer;
mod sauce;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

use clap::Parser;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use atlas::FontAtlasRegistry;
use cell::Row;
use renderer::Renderer;

#[derive(Parser)]
#[command(name = "bbsaver", about = "ANSI art pack screensaver")]
struct Cli {
    /// Path or URL to art pack (directory, ZIP file, or https://...)
    #[arg(long)]
    pack: String,

    /// Simulated baud rate
    #[arg(long, default_value_t = 9600)]
    baud: u32,

    /// Launch in fullscreen mode
    #[arg(long)]
    fullscreen: bool,

    /// Smooth sub-pixel scrolling instead of row-by-row stepping
    #[arg(long)]
    smooth: bool,
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    renderer: Option<Renderer>,
    rows: Vec<Row>,
    cols: usize,
    scroll_offset: f64,
    row_accumulator: f64,
    rows_per_sec: f64,
    last_frame: Option<Instant>,
    fullscreen: bool,
    smooth: bool,
    pack: String,
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

impl App {
    fn new(pack: String, baud: u32, fullscreen: bool, smooth: bool) -> Self {
        let rows_per_sec = baud as f64 / 10.0 / 80.0;
        Self {
            window: None,
            gpu: None,
            renderer: None,
            rows: Vec::new(),
            cols: 80,
            scroll_offset: 0.0,
            row_accumulator: 0.0,
            rows_per_sec,
            last_frame: None,
            fullscreen,
            smooth,
            pack,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attrs = Window::default_attributes()
            .with_title("bbsaver")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

        if self.fullscreen {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }

        let window = Arc::new(event_loop.create_window(attrs).unwrap());

        let gpu = pollster::block_on(init_gpu(window.clone()));
        let registry = FontAtlasRegistry::new(&gpu.device, &gpu.queue);
        let renderer = Renderer::new(&gpu.device, gpu.config.format, registry.default());

        // Load pack now that we know the viewport size
        // Use 80 cols for initial viewport calc; will be updated after pack loads
        let viewport_rows = Renderer::viewport_rows(gpu.config.width, gpu.config.height, 80);
        let pack_data = pack::load_pack(&self.pack, viewport_rows);
        self.rows = pack_data.rows;
        self.cols = pack_data.cols;

        eprintln!("Total {} rows, {} cols, rows/sec={:.1}", self.rows.len(), self.cols, self.rows_per_sec);

        self.gpu = Some(gpu);
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.last_frame = Some(Instant::now());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    if let Key::Named(NamedKey::Escape) = event.logical_key {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    if new_size.width > 0 && new_size.height > 0 {
                        gpu.config.width = new_size.width;
                        gpu.config.height = new_size.height;
                        gpu.surface.configure(&gpu.device, &gpu.config);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if SHOULD_EXIT.load(Ordering::Relaxed) {
                    event_loop.exit();
                    return;
                }

                if let Some(last) = self.last_frame {
                    let now = Instant::now();
                    let elapsed = now.duration_since(last).as_secs_f64();
                    self.last_frame = Some(now);

                    if !self.rows.is_empty() {
                        let total = self.rows.len() as f64;
                        if self.smooth {
                            self.scroll_offset += elapsed * self.rows_per_sec;
                        } else {
                            // Step whole rows: accumulate time, advance when a full row is due
                            self.row_accumulator += elapsed * self.rows_per_sec;
                            let steps = self.row_accumulator.floor();
                            self.scroll_offset += steps;
                            self.row_accumulator -= steps;
                        }
                        if self.scroll_offset >= total {
                            self.scroll_offset -= total;
                        }
                    }
                }

                if let (Some(gpu), Some(renderer)) = (&self.gpu, &self.renderer) {
                    render_frame(gpu, renderer, &self.rows, self.cols, self.scroll_offset);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

async fn init_gpu(window: Arc<Window>) -> GpuState {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

    let surface = instance.create_surface(window.clone()).unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .unwrap();

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("bbsaver"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })
        .await
        .unwrap();

    let size = window.inner_size();
    let caps = surface.get_capabilities(&adapter);
    let format = caps.formats[0];

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    GpuState {
        surface,
        device,
        queue,
        config,
    }
}

fn render_frame(gpu: &GpuState, renderer: &Renderer, rows: &[Row], cols: usize, scroll_offset: f64) {
    let output = match gpu.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(tex)
        | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
        _ => return,
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render"),
        });

    renderer.render(
        &gpu.queue,
        &view,
        &mut encoder,
        rows,
        cols,
        scroll_offset,
        [gpu.config.width, gpu.config.height],
    );

    gpu.queue.submit(std::iter::once(encoder.finish()));
    output.present();
}

fn install_signal_handler() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGTERM, handle_sigterm as *const () as libc::sighandler_t);
    }
}

#[cfg(unix)]
extern "C" fn handle_sigterm(_: libc::c_int) {
    SHOULD_EXIT.store(true, Ordering::Relaxed);
}

fn main() {
    env_logger::init();
    install_signal_handler();

    let cli = Cli::parse();

    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new(cli.pack, cli.baud, cli.fullscreen, cli.smooth);
    event_loop.run_app(&mut app).unwrap();
}
