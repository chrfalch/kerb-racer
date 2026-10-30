//! Kerb Racers' render hooks (app.json `game.render`): a depth of field focused
//! on the player's car, a warm grade and a vignette, in one HDR post pass.
//! Presentation only: the hooks read the world and never step or change it.
use exact_game::{AmbientOcclusion, Vec3};
use exact_game_render::hooks::{Pipelines, PostInputs};
use exact_game_render::{FrameView, HookGpu, Hooks, Needs, RenderError, RenderWorld};
use exact_gpu::wgpu;

/// The shader inventory (game.render.shaders), reflected at build.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

struct Gpu {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

#[derive(Default)]
pub struct Lens {
    gpu: Pipelines<Gpu>,
    uniform: Option<wgpu::Buffer>,
    device: Option<wgpu::Device>,
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
        self.on = world.resource::<AmbientOcclusion>().is_some();
        if !self.on || exact_gpu::shaders::shader_source(shaders::lens::NAME).is_none() {
            return Ok(());
        }
        let format = view.color_format;
        self.gpu.update(gpu.device, view.time.shader_generation, || {
            let layout = gpu.device.create_bind_group_layout(&shaders::lens::GROUP_0);
            let module = gpu.device.create_shader_module(shaders::lens::module());
            let pipeline = gpu
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("lens"),
                    layout: Some(&gpu.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("lens"),
                            bind_group_layouts: &[Some(&layout)],
                            immediate_size: 0,
                        },
                    )),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some(shaders::lens::entry::LENS_VS),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(shaders::lens::entry::LENS_FS),
                        targets: &[Some(format.into())],
                        compilation_options: Default::default(),
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                });
            Gpu { pipeline, layout }
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
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lens"),
            layout: &gpu.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lens"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: inputs.output,
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
        });
        pass.set_viewport(0., 0., view.pixels.0 as f32, view.pixels.1 as f32, 0., 1.);
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
