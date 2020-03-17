use super::{
    state_descriptors::{
        BlendDescriptor, ColorStateDescriptor, ColorWrite, CompareFunction, CullMode,
        DepthStencilStateDescriptor, FrontFace, PrimitiveTopology, RasterizationStateDescriptor,
        StencilStateFaceDescriptor, IndexFormat,
    },
    BindGroup, PipelineLayout, VertexBufferDescriptor,
};
use crate::{
    asset::{AssetStorage, Handle},
    render::{
        render_resource::resource_name,
        shader::{Shader, ShaderStages},
        texture::TextureFormat,
    },
};

#[derive(Clone, Debug)]
pub enum PipelineLayoutType {
    Manual(PipelineLayout),
    Reflected(Option<PipelineLayout>),
}

#[derive(Clone, Debug)]
pub enum DescriptorType<T> {
    Manual(T),
    Reflected(Option<T>),
}

#[derive(Clone, Debug)]
pub struct PipelineDescriptor {
    pub name: Option<String>,
    pub draw_targets: Vec<String>,
    pub layout: PipelineLayoutType,
    pub shader_stages: ShaderStages,
    pub reflect_vertex_buffer_descriptors: bool,
    pub rasterization_state: Option<RasterizationStateDescriptor>,

    /// The primitive topology used to interpret vertices.
    pub primitive_topology: PrimitiveTopology,

    /// The effect of draw calls on the color aspect of the output target.
    pub color_states: Vec<ColorStateDescriptor>,

    /// The effect of draw calls on the depth and stencil aspects of the output target, if any.
    pub depth_stencil_state: Option<DepthStencilStateDescriptor>,

    /// The format of any index buffers used with this pipeline.
    pub index_format: IndexFormat,

    /// The format of any vertex buffers used with this pipeline.
    pub vertex_buffer_descriptors: Vec<VertexBufferDescriptor>,

    /// The number of samples calculated per pixel (for MSAA).
    pub sample_count: u32,

    /// Bitmask that restricts the samples of a pixel modified by this pipeline.
    pub sample_mask: u32,

    /// When enabled, produces another sample mask per pixel based on the alpha output value, that
    /// is ANDed with the sample_mask and the primitive coverage to restrict the set of samples
    /// affected by a primitive.
    /// The implicit mask produced for alpha of zero is guaranteed to be zero, and for alpha of one
    /// is guaranteed to be all 1-s.
    pub alpha_to_coverage_enabled: bool,
}

impl PipelineDescriptor {
    fn new(name: Option<&str>, vertex_shader: Handle<Shader>) -> Self {
        PipelineDescriptor {
            name: name.map(|name| name.to_string()),
            layout: PipelineLayoutType::Reflected(None),
            color_states: Vec::new(),
            depth_stencil_state: None,
            draw_targets: Vec::new(),
            shader_stages: ShaderStages::new(vertex_shader),
            vertex_buffer_descriptors: Vec::new(),
            rasterization_state: Some(RasterizationStateDescriptor {
                front_face: FrontFace::Ccw,
                cull_mode: CullMode::Back,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            reflect_vertex_buffer_descriptors: true,
            primitive_topology: PrimitiveTopology::TriangleList,
            index_format: IndexFormat::Uint16,
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        }
    }

    pub fn get_layout(&self) -> Option<&PipelineLayout> {
        match self.layout {
            PipelineLayoutType::Reflected(ref layout) => layout.as_ref(),
            PipelineLayoutType::Manual(ref layout) => Some(layout),
        }
    }

    pub fn get_layout_mut(&mut self) -> Option<&mut PipelineLayout> {
        match self.layout {
            PipelineLayoutType::Reflected(ref mut layout) => layout.as_mut(),
            PipelineLayoutType::Manual(ref mut layout) => Some(layout),
        }
    }
}

impl PipelineDescriptor {
    pub fn build<'a>(
        name: &str,
        shader_storage: &'a mut AssetStorage<Shader>,
        vertex_shader: Shader,
    ) -> PipelineBuilder<'a> {
        PipelineBuilder::new(name, shader_storage, vertex_shader)
    }
}

pub struct PipelineBuilder<'a> {
    pipeline: PipelineDescriptor,
    shader_storage: &'a mut AssetStorage<Shader>,
}

impl<'a> PipelineBuilder<'a> {
    pub fn new(
        name: &str,
        shader_storage: &'a mut AssetStorage<Shader>,
        vertex_shader: Shader,
    ) -> Self {
        let vertex_shader_handle = shader_storage.add(vertex_shader);
        PipelineBuilder {
            pipeline: PipelineDescriptor::new(Some(name), vertex_shader_handle),
            shader_storage,
        }
    }

    pub fn finish(self) -> PipelineDescriptor {
        self.pipeline
    }

    pub fn with_fragment_shader(mut self, fragment_shader: Shader) -> Self {
        let fragment_shader_handle = self.shader_storage.add(fragment_shader);
        self.pipeline.shader_stages.fragment = Some(fragment_shader_handle);
        self
    }

    pub fn add_color_state(mut self, color_state_descriptor: ColorStateDescriptor) -> Self {
        self.pipeline.color_states.push(color_state_descriptor);
        self
    }

    pub fn with_depth_stencil_state(
        mut self,
        depth_stencil_state: DepthStencilStateDescriptor,
    ) -> Self {
        if let Some(_) = self.pipeline.depth_stencil_state {
            panic!("Depth stencil state has already been set");
        }
        self.pipeline.depth_stencil_state = Some(depth_stencil_state);
        self
    }

    pub fn add_bind_group(mut self, bind_group: BindGroup) -> Self {
        if let PipelineLayoutType::Reflected(_) = self.pipeline.layout {
            self.pipeline.layout = PipelineLayoutType::Manual(PipelineLayout::new());
        }

        if let PipelineLayoutType::Manual(ref mut layout) = self.pipeline.layout {
            layout.bind_groups.push(bind_group);
        }

        self
    }

    pub fn add_vertex_buffer_descriptor(
        mut self,
        vertex_buffer_descriptor: VertexBufferDescriptor,
    ) -> Self {
        self.pipeline.reflect_vertex_buffer_descriptors = false;
        self.pipeline
            .vertex_buffer_descriptors
            .push(vertex_buffer_descriptor);
        self
    }

    pub fn with_index_format(mut self, index_format: IndexFormat) -> Self {
        self.pipeline.index_format = index_format;
        self
    }

    pub fn add_draw_target(mut self, name: &str) -> Self {
        self.pipeline.draw_targets.push(name.to_string());
        self
    }

    pub fn with_rasterization_state(
        mut self,
        rasterization_state: RasterizationStateDescriptor,
    ) -> Self {
        self.pipeline.rasterization_state = Some(rasterization_state);
        self
    }

    pub fn with_primitive_topology(mut self, primitive_topology: PrimitiveTopology) -> Self {
        self.pipeline.primitive_topology = primitive_topology;
        self
    }

    pub fn with_sample_count(mut self, sample_count: u32) -> Self {
        self.pipeline.sample_count = sample_count;
        self
    }

    pub fn with_alpha_to_coverage_enabled(mut self, alpha_to_coverage_enabled: bool) -> Self {
        self.pipeline.alpha_to_coverage_enabled = alpha_to_coverage_enabled;
        self
    }

    pub fn with_sample_mask(mut self, sample_mask: u32) -> Self {
        self.pipeline.sample_mask = sample_mask;
        self
    }

    pub fn with_standard_config(self) -> Self {
        self.with_depth_stencil_state(DepthStencilStateDescriptor {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: CompareFunction::Less,
            stencil_front: StencilStateFaceDescriptor::IGNORE,
            stencil_back: StencilStateFaceDescriptor::IGNORE,
            stencil_read_mask: 0,
            stencil_write_mask: 0,
        })
        .add_color_state(ColorStateDescriptor {
            format: TextureFormat::Bgra8UnormSrgb,
            color_blend: BlendDescriptor::REPLACE,
            alpha_blend: BlendDescriptor::REPLACE,
            write_mask: ColorWrite::ALL,
        })
        .add_draw_target(resource_name::draw_target::ASSIGNED_MESHES)
    }
}
