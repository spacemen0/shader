use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{self, WindowBuilder},
};

// Vertex shader to transform vertices
const VERTEX_SHADER: &str = r#"
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}
"#;

// Fragment shader for psychedelic effects with added grain
const FRAGMENT_SHADER: &str = r#"
@group(0) @binding(0)
var<uniform> time: f32;

// Hash function for pseudo-random numbers
fn hash(p: vec2<f32>) -> f32 {
    var h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

// Noise function
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let resolution = vec2<f32>(1980.0, 1200.0);
    let position = pos.xy / resolution;
    
    // Circular waves
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(position, center);
    
    // Psychedelic color mixing
    let r = sin(position.x * 10.0 + time * 0.1) * 0.5 + 0.5;
    let g = cos(position.y * 8.0 - time * 0.2) * 0.5 + 0.5;
    let b = sin(dist * 15.0 - time * 0.3) * 0.5 + 0.5;
    
    // Warping effect
    let warp = sin(position.x * 5.0 + time) * cos(position.y * 5.0 + time * 0.2) * 0.1;
    let warp_pos = position + vec2<f32>(warp, warp);
    
    // Spiral patterns
    let angle = atan2(warp_pos.y - 0.5, warp_pos.x - 0.5);
    let spiral = sin(dist * 20.0 + angle * 5.0 + time * 0.2) * 0.5 + 0.5;
    
    // Grain effect - high frequency noise
    let grain_intensity = 0.05; // Adjust for more/less grain
    let grain_speed = 5.0; // How quickly the grain pattern changes
    
    // Animated grain with time
    let grain_pos = pos.xy + time * grain_speed;
    let grain = noise(grain_pos * 20.0) * 2.0 - 1.0;
    
    // Final color mixing
    let color = vec3<f32>(
        r * spiral + 0.2 * sin(time * 0.2 + position.x * 5.0),
        g * spiral + 0.2 * cos(time * 0.3 + position.y * 3.0),
        b * spiral + 0.2 * sin(time * 0.1 + dist * 10.0)
    );
    
    // Apply grain to color
    let color_with_grain = color + vec3<f32>(grain * grain_intensity);
    
    // Pulsing effect
    let pulse = sin(time * 0.2) * 0.1 + 0.9;
    
    return vec4<f32>(color_with_grain * pulse, 1.0);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TimeUniform {
    time: f32,
}

fn main() {
    // Set up the window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Psychedelic WGPU Shader")
        .with_fullscreen(Some(window::Fullscreen::Borderless(None)))
        .build(&event_loop)
        .unwrap();

    // Set up the GPU instance
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        dx12_shader_compiler: Default::default(),
    });

    // Connect to the GPU surface
    let surface = unsafe { instance.create_surface(&window) }.unwrap();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .unwrap();

    // Create the device and command queue
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
        },
        None,
    ))
    .unwrap();

    // Configure the surface
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .unwrap_or(&surface_caps.formats[0]);

    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: *surface_format,
        width: window.inner_size().width,
        height: window.inner_size().height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    // Create the uniform buffer for time
    let time_uniform = TimeUniform { time: 0.0 };
    let time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Time Buffer"),
        contents: bytemuck::cast_slice(&[time_uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    // Create the bind group layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
        label: Some("bind_group_layout"),
    });

    // Create the bind group
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: time_buffer.as_entire_binding(),
        }],
        label: Some("bind_group"),
    });

    // Create the shader module
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(VERTEX_SHADER.into()),
    });

    let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Fragment Shader"),
        source: wgpu::ShaderSource::Wgsl(FRAGMENT_SHADER.into()),
    });

    // Create the render pipeline
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &fragment_shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    });

    // Timer for animation
    let start_time = Instant::now();

    // Run the event loop
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(physical_size) => {
                    config.width = physical_size.width;
                    config.height = physical_size.height;
                    surface.configure(&device, &config);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    config.width = new_inner_size.width;
                    config.height = new_inner_size.height;
                    surface.configure(&device, &config);
                }
                _ => {}
            },
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                let elapsed = start_time.elapsed().as_secs_f32();
                queue.write_buffer(
                    &time_buffer,
                    0,
                    bytemuck::cast_slice(&[TimeUniform { time: elapsed }]),
                );

                let output = surface.get_current_texture().unwrap();
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.2,
                                    b: 0.3,
                                    a: 1.0,
                                }),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    });

                    render_pass.set_pipeline(&render_pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.draw(0..3, 0..1);
                }

                queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}
