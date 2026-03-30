mod ansi;
mod atlas;
mod cell;
mod cp437;
mod pack;
mod renderer;
mod sauce;

use std::collections::HashMap;
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

    /// Show on all monitors (only with --fullscreen)
    #[arg(long)]
    all_monitors: bool,
}

/// Per-window state (each monitor gets one).
struct WindowState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

struct App {
    // Shared GPU state
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,

    // Per-window state
    windows: HashMap<WindowId, WindowState>,

    // Shared content
    rows: Vec<Row>,
    cols: usize,
    reference_width: u32,

    // Scroll state
    scroll_offset: f64,
    row_accumulator: f64,
    rows_per_sec: f64,
    last_frame: Option<Instant>,

    // Config
    fullscreen: bool,
    smooth: bool,
    all_monitors: bool,
    pack: String,
    initialized: bool,
}

impl App {
    fn new(pack: String, baud: u32, fullscreen: bool, smooth: bool, all_monitors: bool) -> Self {
        let rows_per_sec = baud as f64 / 10.0 / 80.0;
        Self {
            device: None,
            queue: None,
            windows: HashMap::new(),
            rows: Vec::new(),
            cols: 80,
            reference_width: 0,
            scroll_offset: 0.0,
            row_accumulator: 0.0,
            rows_per_sec,
            last_frame: None,
            fullscreen,
            smooth,
            all_monitors,
            pack,
            initialized: false,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        // Create GPU instance and adapter (shared across all windows)
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        if self.fullscreen && self.all_monitors {
            // One fullscreen window per monitor
            let monitors: Vec<_> = event_loop.available_monitors().collect();
            let mut windows = Vec::new();
            for monitor in &monitors {
                let window = Arc::new(
                    event_loop
                        .create_window(
                            Window::default_attributes()
                                .with_title("bbsaver")
                                .with_fullscreen(Some(
                                    winit::window::Fullscreen::Borderless(Some(monitor.clone())),
                                )),
                        )
                        .unwrap(),
                );
                windows.push(window);
            }
            if windows.is_empty() {
                // Fallback: single borderless fullscreen
                windows.push(Arc::new(
                    event_loop
                        .create_window(
                            Window::default_attributes()
                                .with_title("bbsaver")
                                .with_fullscreen(Some(
                                    winit::window::Fullscreen::Borderless(None),
                                )),
                        )
                        .unwrap(),
                ));
            }
            self.init_gpu_and_windows(&instance, windows);
        } else if self.fullscreen {
            // Single fullscreen window on primary monitor
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("bbsaver")
                            .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None))),
                    )
                    .unwrap(),
            );
            self.init_gpu_and_windows(&instance, vec![window]);
        } else {
            // Single windowed mode
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("bbsaver")
                            .with_inner_size(winit::dpi::LogicalSize::new(800, 600)),
                    )
                    .unwrap(),
            );
            self.init_gpu_and_windows(&instance, vec![window]);
        }

        // Cell size is derived from the narrowest screen so art fills it edge-to-edge.
        // Wider screens get centered black bars.
        self.reference_width = self
            .windows
            .values()
            .map(|ws| ws.config.width)
            .min()
            .unwrap_or(800);

        // For pack loading, use the tallest screen's height to ensure enough rows
        let max_height = self
            .windows
            .values()
            .map(|ws| ws.config.height)
            .max()
            .unwrap_or(600);
        let viewport_rows = Renderer::viewport_rows(max_height, self.reference_width, 80);
        let pack_data = pack::load_pack(&self.pack, viewport_rows);
        self.rows = pack_data.rows;
        self.cols = pack_data.cols;

        eprintln!(
            "{} monitor(s), {} rows, {} cols, rows/sec={:.1}",
            self.windows.len(),
            self.rows.len(),
            self.cols,
            self.rows_per_sec
        );

        self.last_frame = Some(Instant::now());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
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
                if let Some(ws) = self.windows.get_mut(&id) {
                    if new_size.width > 0 && new_size.height > 0 {
                        ws.config.width = new_size.width;
                        ws.config.height = new_size.height;
                        if let Some(device) = &self.device {
                            ws.surface.configure(device, &ws.config);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if SHOULD_EXIT.load(Ordering::Relaxed) {
                    event_loop.exit();
                    return;
                }

                // Only advance scroll once per frame (first window to request redraw)
                if let Some(first_id) = self.windows.keys().next().copied() {
                    if id == first_id {
                        if let Some(last) = self.last_frame {
                            let now = Instant::now();
                            let elapsed = now.duration_since(last).as_secs_f64();
                            self.last_frame = Some(now);

                            if !self.rows.is_empty() {
                                let total = self.rows.len() as f64;
                                if self.smooth {
                                    self.scroll_offset += elapsed * self.rows_per_sec;
                                } else {
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
                    }
                }

                // Render this window
                if let (Some(device), Some(queue), Some(ws)) =
                    (&self.device, &self.queue, self.windows.get(&id))
                {
                    render_window(
                        device,
                        queue,
                        ws,
                        &self.rows,
                        self.cols,
                        self.scroll_offset,
                        self.reference_width,
                    );
                }

                // Request redraw on all windows
                for ws in self.windows.values() {
                    ws.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

impl App {
    fn init_gpu_and_windows(
        &mut self,
        instance: &wgpu::Instance,
        windows: Vec<Arc<Window>>,
    ) {
        // Create surface from first window to find a compatible adapter
        let first_surface = instance.create_surface(windows[0].clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&first_surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("bbsaver"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
        ))
        .unwrap();

        let registry = atlas::FontAtlasRegistry::new(&device, &queue);

        // Set up each window. We already created first_surface, so consume it for window 0
        // and create new surfaces for the rest.
        let mut first_surface = Some(first_surface);
        for (i, window) in windows.iter().enumerate() {
            let surface = if i == 0 {
                first_surface.take().unwrap()
            } else {
                instance.create_surface(window.clone()).unwrap()
            };

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

            let renderer = Renderer::new(&device, format, registry.default());

            self.windows.insert(
                window.id(),
                WindowState {
                    window: window.clone(),
                    surface,
                    config,
                    renderer,
                },
            );
        }

        self.device = Some(device);
        self.queue = Some(queue);
    }
}

fn render_window(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ws: &WindowState,
    rows: &[Row],
    cols: usize,
    scroll_offset: f64,
    reference_width: u32,
) {
    let output = match ws.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(tex)
        | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
        _ => return,
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("render"),
    });

    ws.renderer.render(
        queue,
        &view,
        &mut encoder,
        rows,
        cols,
        scroll_offset,
        [ws.config.width, ws.config.height],
        reference_width,
    );

    queue.submit(std::iter::once(encoder.finish()));
    output.present();
}

fn install_signal_handler() {
    #[cfg(unix)]
    unsafe {
        libc::signal(
            libc::SIGTERM,
            handle_sigterm as *const () as libc::sighandler_t,
        );
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
    let mut app = App::new(cli.pack, cli.baud, cli.fullscreen, cli.smooth, cli.all_monitors);
    event_loop.run_app(&mut app).unwrap();
}
