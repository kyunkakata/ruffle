// This is a new lint with false positives, see https://github.com/rust-lang/rust-clippy/issues/10318
#![allow(clippy::extra_unused_type_parameters)]

use crate::bitmaps::BitmapSamplers;
use crate::descriptors::Quad;
use crate::mesh::BitmapBinds;
use crate::pipelines::Pipelines;
use crate::target::{RenderTarget, SwapChainTarget};
use crate::uniform_buffer::UniformBuffer;
use crate::utils::{
    capture_image, create_buffer_with_data, format_list, get_backend_names, BufferDimensions,
};
use bytemuck::{Pod, Zeroable};
use descriptors::Descriptors;
use enum_map::Enum;
use once_cell::sync::OnceCell;
use ruffle_render::bitmap::{BitmapHandle, BitmapHandleImpl, RgbaBufRead, SyncHandle};
use ruffle_render::color_transform::ColorTransform;
use ruffle_render::tessellator::{Gradient as TessGradient, GradientType, Vertex as TessVertex};
use std::cell::Cell;
use std::sync::Arc;
use swf::GradientSpread;
pub use wgpu;

type Error = Box<dyn std::error::Error>;

#[macro_use]
mod utils;

mod bitmaps;
mod context3d;
mod globals;
mod pipelines;
pub mod target;
mod uniform_buffer;

pub mod backend;
mod blend;
mod buffer_builder;
mod buffer_pool;
#[cfg(feature = "clap")]
pub mod clap;
pub mod descriptors;
mod layouts;
mod mesh;
mod shaders;
mod surface;

impl BitmapHandleImpl for Texture {}

pub fn as_texture(handle: &BitmapHandle) -> &Texture {
    <dyn BitmapHandleImpl>::downcast_ref(&*handle.0).unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum MaskState {
    NoMask,
    DrawMaskStencil,
    DrawMaskedContent,
    ClearMaskStencil,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct PushConstants {
    transforms: Transforms,
    colors: ColorAdjustments,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Transforms {
    world_matrix: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TextureTransforms {
    u_matrix: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, PartialEq)]
pub struct ColorAdjustments {
    mult_color: [f32; 4],
    add_color: [f32; 4],
}

pub const DEFAULT_COLOR_ADJUSTMENTS: ColorAdjustments = ColorAdjustments {
    mult_color: [1.0, 1.0, 1.0, 1.0],
    add_color: [0.0, 0.0, 0.0, 0.0],
};

impl From<ColorTransform> for ColorAdjustments {
    fn from(transform: ColorTransform) -> Self {
        if transform == ColorTransform::IDENTITY {
            DEFAULT_COLOR_ADJUSTMENTS
        } else {
            Self {
                mult_color: transform.mult_rgba_normalized(),
                add_color: transform.add_rgba_normalized(),
            }
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct PosVertex {
    position: [f32; 2],
}

impl From<TessVertex> for PosVertex {
    fn from(vertex: TessVertex) -> Self {
        Self {
            position: [vertex.x, vertex.y],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct PosColorVertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl From<TessVertex> for PosColorVertex {
    fn from(vertex: TessVertex) -> Self {
        Self {
            position: [vertex.x, vertex.y],
            color: [
                f32::from(vertex.color.r) / 255.0,
                f32::from(vertex.color.g) / 255.0,
                f32::from(vertex.color.b) / 255.0,
                f32::from(vertex.color.a) / 255.0,
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GradientUniforms {
    focal_point: f32,
    interpolation: i32,
    shape: i32,
    repeat: i32,
}

impl From<TessGradient> for GradientUniforms {
    fn from(gradient: TessGradient) -> Self {
        Self {
            focal_point: gradient.focal_point.to_f32().clamp(-0.98, 0.98),
            interpolation: (gradient.interpolation == swf::GradientInterpolation::LinearRgb) as i32,
            shape: match gradient.gradient_type {
                GradientType::Linear => 1,
                GradientType::Radial => 2,
                GradientType::Focal => 3,
            },
            repeat: match gradient.repeat_mode {
                GradientSpread::Pad => 1,
                GradientSpread::Reflect => 2,
                GradientSpread::Repeat => 3,
            },
        }
    }
}

#[derive(Debug)]
pub enum QueueSyncHandle {
    AlreadyCopied {
        index: wgpu::SubmissionIndex,
        buffer: Arc<wgpu::Buffer>,
        buffer_dimensions: BufferDimensions,
        descriptors: Arc<Descriptors>,
    },
    NotCopied {
        handle: BitmapHandle,
        size: wgpu::Extent3d,
        descriptors: Arc<Descriptors>,
    },
}

impl SyncHandle for QueueSyncHandle {
    fn retrieve_offscreen_texture(
        self: Box<Self>,
        with_rgba: RgbaBufRead,
    ) -> Result<(), ruffle_render::error::Error> {
        self.capture(with_rgba);
        Ok(())
    }
}

impl QueueSyncHandle {
    pub fn capture<R, F: FnOnce(&[u8], u32) -> R>(self, with_rgba: F) -> R {
        match self {
            QueueSyncHandle::AlreadyCopied {
                index,
                buffer,
                buffer_dimensions,
                descriptors,
            } => capture_image(
                &descriptors.device,
                &buffer,
                &buffer_dimensions,
                Some(index),
                with_rgba,
            ),
            QueueSyncHandle::NotCopied {
                handle,
                size,
                descriptors,
            } => {
                let texture = as_texture(&handle);

                let buffer_label = create_debug_label!("Render target buffer");
                let buffer_dimensions =
                    BufferDimensions::new(size.width as usize, size.height as usize);
                let buffer = descriptors.device.create_buffer(&wgpu::BufferDescriptor {
                    label: buffer_label.as_deref(),
                    size: (buffer_dimensions.padded_bytes_per_row.get() as u64
                        * buffer_dimensions.height as u64),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let label = create_debug_label!("Render target transfer encoder");
                let mut encoder =
                    descriptors
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: label.as_deref(),
                        });
                encoder.copy_texture_to_buffer(
                    wgpu::ImageCopyTexture {
                        texture: &texture.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::ImageCopyBuffer {
                        buffer: &buffer,
                        layout: wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(buffer_dimensions.padded_bytes_per_row),
                            rows_per_image: None,
                        },
                    },
                    size,
                );
                let index = descriptors.queue.submit(Some(encoder.finish()));

                let image = capture_image(
                    &descriptors.device,
                    &buffer,
                    &buffer_dimensions,
                    Some(index),
                    with_rgba,
                );

                // After we've read pixels from a texture enough times, we'll store this buffer so that
                // future reads will be faster (it'll copy as part of the draw process instead)
                texture.copy_count.set(texture.copy_count.get() + 1);
                if texture.copy_count.get() >= 2 {
                    let _ = texture.texture_offscreen.set(TextureOffscreen {
                        buffer: Arc::new(buffer),
                        buffer_dimensions,
                    });
                }

                image
            }
        }
    }
}

#[derive(Debug)]
pub struct Texture {
    pub(crate) texture: Arc<wgpu::Texture>,
    bind_linear: OnceCell<BitmapBinds>,
    bind_nearest: OnceCell<BitmapBinds>,
    texture_offscreen: OnceCell<TextureOffscreen>,
    copy_count: Cell<u8>,
    width: u32,
    height: u32,
}

impl Texture {
    pub fn bind_group(
        &self,
        smoothed: bool,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        quad: &Quad,
        handle: BitmapHandle,
        samplers: &BitmapSamplers,
    ) -> &BitmapBinds {
        let bind = match smoothed {
            true => &self.bind_linear,
            false => &self.bind_nearest,
        };
        bind.get_or_init(|| {
            BitmapBinds::new(
                device,
                layout,
                samplers.get_sampler(false, smoothed),
                &quad.texture_transforms,
                0 as wgpu::BufferAddress,
                self.texture.create_view(&Default::default()),
                create_debug_label!("Bitmap {:?} bind group (smoothed: {})", handle.0, smoothed),
            )
        })
    }
}

#[derive(Debug)]
struct TextureOffscreen {
    buffer: Arc<wgpu::Buffer>,
    buffer_dimensions: BufferDimensions,
}
