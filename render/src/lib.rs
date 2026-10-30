//! Kerb Racers' render hooks (app.json `game.render`): a depth of field focused
//! on the player's car, a warm grade and a vignette, in two HDR post passes: the
//! blur gathered at half resolution, then the grade over the full frame.
//! Presentation only: the hooks read the world and never step or change it.
use exact_game::{AmbientOcclusion, Vec3};
use exact_game_render::hooks::{Pipelines, PostInputs};
use exact_game_render::{FrameView, HookGpu, Hooks, Needs, RenderError, RenderWorld};
use exact_gpu::wgpu;

/// The shader inventory (game.render.shaders), reflected at build.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

/// The half-resolution blur: linear HDR, filterable for the grade's bilinear fetch.
const BLUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

struct Gpu {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    blur: wgpu::RenderPipeline,
    blur_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// The half-resolution blur target, kept until the frame outgrows it.
struct Half {
    size: (u32, u32),
    view: wgpu::TextureView,
}

fn fullscreen(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    module: &wgpu::ShaderModule,
    (vs, fs): (&str, &str),
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            }),
        ),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(vs),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fs),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

#[derive(Default)]
pub struct Lens {
    gpu: Pipelines<Gpu>,
    uniform: Option<wgpu::Buffer>,
    device: Option<wgpu::Device>,
    half: Option<Half>,
    /// Off at low quality, which runs without the full-screen passes.
    on: bool,
}

impl Hooks for Lens {
    fn prepare(
        &mut self,
        gpu: &HookGpu<'_>,
        view: &FrameView<'_>,
        world: &RenderWorld<'_>,
    ) -> Result<(), RenderError> {
        // Medium and high quality turn on ambient occlusion; the lens comes with it.
        self.on = (world.resource::<AmbientOcclusion>().is_some() || option_env!("KR_LENS_FORCE").is_some()) && option_env!("KR_NOLENS").is_none();
        if !self.on
            || exact_gpu::shaders::shader_source(shaders::lens::NAME).is_none()
            || exact_gpu::shaders::shader_source(shaders::lens_blur::NAME).is_none()
        {
            return Ok(());
        }
        let format = view.color_format;
        self.gpu.update(gpu.device, view.time.shader_generation, || {
            let device = gpu.device;
            let layout = device.create_bind_group_layout(&shaders::lens::GROUP_0);
            let module = device.create_shader_module(shaders::lens::module());
            let pipeline = fullscreen(
                device,
                "lens",
                &layout,
                &module,
                (shaders::lens::entry::LENS_VS, shaders::lens::entry::LENS_FS),
                format,
            );
            let blur_layout = device.create_bind_group_layout(&shaders::lens_blur::GROUP_0);
            let module = device.create_shader_module(shaders::lens_blur::module());
            let blur = fullscreen(
                device,
                "lens blur",
                &blur_layout,
                &module,
                (
                    shaders::lens_blur::entry::BLUR_VS,
                    shaders::lens_blur::entry::BLUR_FS,
                ),
                BLUR_FORMAT,
            );
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("lens blur"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            Gpu {
                pipeline,
                layout,
                blur,
                blur_layout,
                sampler,
            }
        });
        let uniform = self.uniform.get_or_insert_with(|| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lens"),
                size: 48,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        // Focus on the player's car: its distance along the camera's view axis,
        // and the screen row it sits on, where the sharp band centres.
        let car = world
            .named("player")
            .and_then(|car| view.pose(car).or(world.global(car)))
            .map(|car| car.w_axis.truncate() + Vec3::Y * 0.5);
        let (focus, band) = match car {
            Some(car) => {
                let seen = view.frame.view.transform_point3(car);
                let clip = view.frame.proj.project_point3(seen);
                (-seen.z, (0.5 - clip.y * 0.5).clamp(0.2, 0.8))
            }
            None => (41.0, 0.5),
        };
        let (w, h) = (view.pixels.0 as f32, view.pixels.1 as f32);
        // The blur scales with the picture, so every size looks the same.
        let words = [
            focus,
            5.0,
            14.0,
            h * 0.016,
            w,
            h,
            0.36,
            0.05,
            band,
            0.09,
            0.28,
            1.0,
        ];
        gpu.queue
            .write_buffer(uniform, 0, &words.map(f32::to_le_bytes).concat());
        self.device = Some(gpu.device.clone());
        Ok(())
    }
    fn needs(&self) -> Needs {
        match (self.on, self.gpu.pending()) {
            (false, _) => Needs::NONE,
            (true, true) => Needs::PENDING,
            (true, false) => Needs::HDR_POST | Needs::FINAL_DEPTH,
        }
    }
    fn pending_reason(&self) -> &str {
        "lens pipeline validating"
    }
    fn error(&self) -> Option<&str> {
        self.gpu.error()
    }
    fn post(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        inputs: &PostInputs<'_>,
        view: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        let (Some(gpu), Some(uniform), Some(device), Some(depth)) = (
            self.gpu.get(),
            &self.uniform,
            &self.device,
            inputs.depth,
        ) else {
            return Ok(());
        };
        let size = (view.pixels.0.div_ceil(2), view.pixels.1.div_ceil(2));
        if !matches!(&self.half, Some(h) if h.size.0 >= size.0 && h.size.1 >= size.1) {
            {
                let grown = self.half.as_ref().map_or(size, |h| (h.size.0.max(size.0), h.size.1.max(size.1)));
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("lens blur"),
                    size: wgpu::Extent3d {
                        width: grown.0,
                        height: grown.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: BLUR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                self.half = Some(Half {
                    size: grown,
                    view: texture.create_view(&Default::default()),
                });
            }
        }
        let half = self.half.as_ref().expect("blur target");
        let texture = |binding, view| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        };
        let blur_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lens blur"),
            layout: &gpu.blur_layout,
            entries: &[
                texture(0, inputs.input),
                texture(1, depth),
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lens"),
            layout: &gpu.layout,
            entries: &[
                texture(0, inputs.input),
                texture(1, depth),
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
                texture(3, &half.view),
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&gpu.sampler),
                },
            ],
        });
        {
            let mut blur = pass(encoder, "lens blur", &half.view);
            blur.set_viewport(0., 0., size.0 as f32, size.1 as f32, 0., 1.);
            blur.set_pipeline(&gpu.blur);
            blur.set_bind_group(0, &blur_group, &[]);
            blur.draw(0..3, 0..1);
        }
        let mut grade = pass(encoder, "lens", inputs.output);
        grade.set_viewport(0., 0., view.pixels.0 as f32, view.pixels.1 as f32, 0., 1.);
        grade.set_pipeline(&gpu.pipeline);
        grade.set_bind_group(0, &group, &[]);
        grade.draw(0..3, 0..1);
        Ok(())
    }
}
