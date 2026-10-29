use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    platform::android::{EventLoopBuilderExtAndroid, activity::AndroidApp},
    window::{Window, WindowId},
};

struct Renderer {
    _instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

impl Renderer {
    fn new(window: Arc<Window>) -> Result<Self, String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| format!("surface creation failed: {error}"))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .map_err(|error| format!("Vulkan adapter request failed: {error}"))?;
        let info = adapter.get_info();
        log::info!(
            "ANDROID_RENDER adapter backend={:?} name={} device_type={:?} driver={} driver_info={}",
            info.backend,
            info.name,
            info.device_type,
            info.driver,
            info.driver_info
        );

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|error| format!("Vulkan device request failed: {error}"))?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .first()
            .copied()
            .ok_or("surface exposes no formats")?;
        let present_mode = if capabilities
            .present_modes
            .contains(&wgpu::PresentMode::Fifo)
        {
            wgpu::PresentMode::Fifo
        } else {
            *capabilities
                .present_modes
                .first()
                .ok_or("surface exposes no present modes")?
        };
        let alpha_mode = *capabilities
            .alpha_modes
            .first()
            .ok_or("surface exposes no alpha modes")?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::default(),
        };
        surface.configure(&device, &config);
        log::info!(
            "ANDROID_RENDER surface width={} height={} format={:?} present_mode={:?}",
            config.width,
            config.height,
            config.format,
            config.present_mode
        );

        Ok(Self {
            _instance: instance,
            window,
            surface,
            adapter,
            device,
            queue,
            config,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        log::info!("ANDROID_RENDER resized width={width} height={height}");
        self.window.request_redraw();
    }

    fn draw(&mut self) -> Result<(), String> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("surface acquisition failed validation".into());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Horizon Android render check"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("visible Android render check"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.95,
                            g: 0.15,
                            b: 0.55,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        let info = self.adapter.get_info();
        log::info!(
            "PRESENTED_FRAME count=1 backend={:?} adapter={} width={} height={} format={:?}",
            info.backend,
            info.name,
            self.config.width,
            self.config.height,
            self.config.format
        );
        Ok(())
    }
}

#[derive(Default)]
struct RenderCheck {
    renderer: Option<Renderer>,
    error: Option<String>,
}

impl ApplicationHandler for RenderCheck {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        log::info!("ANDROID_RENDER lifecycle=resumed");
        if self.renderer.is_some() {
            return;
        }
        let window = match event_loop.create_window(
            Window::default_attributes().with_title("Horizon Android Vulkan Render Check"),
        ) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.error = Some(format!("window creation failed: {error}"));
                event_loop.exit();
                return;
            }
        };
        match Renderer::new(window) {
            Ok(renderer) => {
                renderer.window.request_redraw();
                self.renderer = Some(renderer);
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        log::info!("ANDROID_RENDER lifecycle=suspended; releasing renderer");
        self.renderer = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer
                    && let Err(error) = renderer.draw()
                {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}

pub fn run(app: AndroidApp) -> Result<(), String> {
    let mut builder = EventLoop::builder();
    builder.with_android_app(app);
    let event_loop = builder
        .build()
        .map_err(|error| format!("event loop creation failed: {error}"))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut render_check = RenderCheck::default();
    event_loop
        .run_app(&mut render_check)
        .map_err(|error| format!("event loop failed: {error}"))?;
    if let Some(error) = render_check.error {
        return Err(error);
    }
    Ok(())
}
