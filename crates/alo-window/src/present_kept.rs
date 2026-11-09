/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The window's pixels kept on the GPU between frames, and only what
//! changed written to them (queue item 873, ADR 0105): the one file that may
//! name `wgpu`, `objc2-core-graphics` and `bytemuck`, and **the one module
//! of the workspace that may write `unsafe`** — one block, in [`tag`].
//!
//! `softbuffer` hands the window server a new image of the whole window for
//! every frame on macOS — 10.4 ms of a letter's 30, at two. Here a texture
//! of the window's size is kept: each frame writes into it only the regions
//! [`crate::conversation_composed::Composed`] composed again, then copies it
//! whole, on the GPU, to the Metal layer's next drawable and presents that
//! — 0.16 ms where `softbuffer` took 11.6–13.2, measured.
//!
//! # The one `unsafe` block, and why it is needed
//!
//! `wgpu` 30 leaves an 8-bit surface's `CAMetalLayer` with no colour space
//! when asked for sRGB, and an untagged layer is not colour-matched at all:
//! sRGB red reaches the display as the display's own red. `softbuffer`'s
//! images are tagged with the device's RGB, which is matched as sRGB. So the
//! layer is tagged the same way after every `configure` (which clears it),
//! and that needs the layer `wgpu` made, which only the `unsafe`
//! [`wgpu::Surface::as_hal`] gives. Setting the colour space on it
//! (`setColorspace`) and making the colour space (`new_device_rgb`) are
//! safe calls. Measured against the machine's Chrome, the tagged layer is
//! nearer Chrome than `softbuffer` on every frame compared (ADR 0105).
//!
//! What would end it: `wgpu` tagging the layer itself for
//! `SurfaceColorSpace::Srgb`, or `softbuffer` keeping a frame on macOS.

use alo_paint::Region;
use objc2_core_graphics::CGColorSpace;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

/// The format of both the kept texture and the drawables: the bytes of the
/// words [`crate::present::packed`] makes, `0x00RRGGBB` little-endian —
/// blue, green, red and a byte the opaque layer does not read.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

/// The GPU, and the window's pixels kept on it.
pub(crate) struct Held {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: Option<(wgpu::Texture, (u32, u32))>,
}

impl Held {
    fn on(adapter: &wgpu::Adapter) -> Result<Self, String> {
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|why| format!("no GPU device: {why}"))?;
        Ok(Self {
            device,
            queue,
            texture: None,
        })
    }

    /// A GPU with no window, for tests of what is written and kept.
    #[cfg(test)]
    pub(crate) fn headless() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|why| format!("no GPU adapter: {why}"))?;
        Self::on(&adapter)
    }

    /// Write `words`, a window of `size` row by row, into the kept texture:
    /// only `regions` of them, or all of them when `regions` is [`None`] or
    /// the texture is of another size (made again). Answers how many bytes
    /// were written.
    pub(crate) fn write(
        &mut self,
        size: (u32, u32),
        words: &[u32],
        regions: Option<&[Region]>,
    ) -> u64 {
        let whole = Region::new(0, 0, size.0, size.1);
        let regions = match (&self.texture, regions) {
            (Some((_, kept)), Some(regions)) if *kept == size => regions,
            _ => {
                self.texture = Some((self.made(size), size));
                whole.as_slice()
            }
        };
        let Some((texture, _)) = &self.texture else {
            return 0;
        };
        let bytes: &[u8] = bytemuck::cast_slice(words);
        let mut written = 0;
        for region in regions {
            let (width, height) = (region.right - region.left, region.bottom - region.top);
            let offset = (u64::from(region.top) * u64::from(size.0) + u64::from(region.left)) * 4;
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: region.left,
                        y: region.top,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset,
                    bytes_per_row: Some(size.0 * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            written += u64::from(width) * u64::from(height) * 4;
        }
        written
    }

    /// A texture of `size` to keep the window's pixels in.
    fn made(&self, (width, height): (u32, u32)) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("the window's pixels, kept"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// The kept texture's words, read back from the GPU, row by row.
    ///
    /// # Errors
    ///
    /// Nothing kept yet, or the GPU would not map what it copied.
    #[cfg(test)]
    pub(crate) fn read_back(&self) -> Result<Vec<u32>, String> {
        let (texture, (width, height)) = self.texture.as_ref().ok_or("nothing kept")?;
        let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("the kept pixels, read back"),
            size: u64::from(row) * u64::from(*height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(*height),
                },
            },
            wgpu::Extent3d {
                width: *width,
                height: *height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tell, heard) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |mapped| {
            let _ = tell.send(mapped);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|why| format!("the GPU was not waited for: {why}"))?;
        heard
            .recv()
            .map_err(|_| "no word of the mapping".to_owned())?
            .map_err(|why| format!("not mapped: {why}"))?;
        let view = buffer
            .get_mapped_range(..)
            .map_err(|why| format!("no view of the mapping: {why}"))?;
        let mut words = Vec::with_capacity((width * height) as usize);
        for line in view.chunks_exact(row as usize) {
            let line = line.get(..(width * 4) as usize).unwrap_or_default();
            words.extend(
                line.chunks_exact(4)
                    .filter_map(|b| <[u8; 4]>::try_from(b).ok())
                    .map(u32::from_le_bytes),
            );
        }
        Ok(words)
    }
}

/// The window's Metal layer, and its pixels kept.
pub(crate) struct Kept {
    surface: wgpu::Surface<'static>,
    held: Held,
    /// The size the surface was last configured at.
    configured: Option<(u32, u32)>,
}

impl std::fmt::Debug for Kept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Kept {:?}", self.configured)
    }
}

impl Kept {
    /// A Metal surface over `window`, and a GPU that can present to it.
    ///
    /// # Errors
    ///
    /// No surface on the window, or no GPU that can draw to it: the window
    /// is then put on screen by `softbuffer` (`crate::present_chosen`).
    pub(crate) fn over<W: Into<wgpu::SurfaceTarget<'static>>>(window: W) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(window)
            .map_err(|why| format!("no Metal surface on the window: {why}"))?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|why| format!("no GPU for the window: {why}"))?;
        let capabilities = surface.get_capabilities(&adapter);
        if !capabilities.formats.contains(&FORMAT)
            || !capabilities.usages.contains(wgpu::TextureUsages::COPY_DST)
            || !capabilities
                .alpha_modes
                .contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            return Err(format!(
                "the window's surface cannot be copied to as {FORMAT:?}, opaque"
            ));
        }
        Ok(Self {
            surface,
            held: Held::on(&adapter)?,
            configured: None,
        })
    }

    /// Show `words`, a window of `size` row by row, writing to the kept
    /// pixels only `regions` of them (all when [`None`]). Answers how many
    /// bytes were written to the GPU.
    ///
    /// A window covered, or not yet shown, gives no drawable: what was
    /// written is kept, and shown with the next frame.
    ///
    /// # Errors
    ///
    /// The surface lost, or tagged, beyond configuring again.
    pub(crate) fn show(
        &mut self,
        size: (u32, u32),
        words: &[u32],
        regions: Option<&[Region]>,
    ) -> Result<u64, String> {
        if size.0 == 0 || size.1 == 0 {
            return Ok(0);
        }
        if self.configured != Some(size) {
            self.configure(size)?;
        }
        let written = self.held.write(size, words, regions);
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(written);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure(size)?;
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    other => return Err(format!("no drawable for the window: {other:?}")),
                }
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("no drawable for the window: the surface is not valid".to_owned());
            }
        };
        if let Some((texture, _)) = &self.held.texture {
            let mut encoder = self
                .held
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            encoder.copy_texture_to_texture(
                texture.as_image_copy(),
                frame.texture.as_image_copy(),
                wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
            );
            self.held.queue.submit([encoder.finish()]);
        }
        self.held.queue.present(frame);
        Ok(written)
    }

    /// The surface made `size`, and its layer tagged again.
    fn configure(&mut self, (width, height): (u32, u32)) -> Result<(), String> {
        self.surface.configure(
            &self.held.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::COPY_DST,
                format: FORMAT,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: Vec::new(),
                color_space: wgpu::SurfaceColorSpace::Srgb,
            },
        );
        tag(&self.surface)?;
        self.configured = Some((width, height));
        Ok(())
    }
}

/// Tag `surface`'s layer with the device's RGB colour space, as
/// `softbuffer` tags its images, so the window server matches its pixels
/// to the display as sRGB.
fn tag(surface: &wgpu::Surface<'_>) -> Result<(), String> {
    let space = CGColorSpace::new_device_rgb().ok_or("no device RGB colour space")?;
    // SAFETY: `wgpu::Surface::as_hal` asks two things (wgpu 30.0.1's
    // documentation). (1) The surface it guards must not be destroyed
    // through the guard: only `CAMetalLayer::setColorspace` is called on
    // the layer, which destroys nothing, and the guard is dropped at the end
    // of this function. (2) Every requirement of `wgpu-hal` is kept: the
    // layer is reached through the hal surface's own `render_layer()`, under
    // its own lock, on the thread that owns the surface, between a
    // `configure` and the next `get_current_texture`, so no drawable of it
    // is out while its colour space is set — the same moment `wgpu-hal`'s
    // `configure` sets it. The colour space is a property Core Animation
    // reads for compositing alone; it changes neither the drawables' format
    // nor their size, which `wgpu` tracks.
    let hal = unsafe { surface.as_hal::<wgpu::hal::api::Metal>() }
        .ok_or("the window's surface is not Metal's")?;
    hal.render_layer().lock().setColorspace(Some(&space));
    Ok(())
}

/// Run `future` on this thread until it is ready: `wgpu`'s requests for an
/// adapter and a device, which on Metal are ready at once.
fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window's words, each its own.
    fn varied(size: (u32, u32), seed: u32) -> Vec<u32> {
        (0..size.0 * size.1)
            .map(|i| (i.wrapping_mul(2_654_435_761) ^ seed) & 0x00ff_ffff)
            .collect()
    }

    #[test]
    fn only_the_regions_given_are_written_and_the_texture_keeps_the_rest() {
        // An odd width: a row's bytes are not a multiple of 256.
        let size = (101, 37);
        let mut held = Held::headless().unwrap_or_else(|why| panic!("{why}"));
        let mut words = varied(size, 1);
        assert_eq!(held.write(size, &words, None), 101 * 37 * 4, "all of it");
        assert_eq!(held.read_back(), Ok(words.clone()));

        let changed = [
            Region::new(3, 4, 13, 9).unwrap_or_else(|| panic!("a region")),
            Region::new(90, 30, 101, 37).unwrap_or_else(|| panic!("a region")),
        ];
        let newer = varied(size, 2);
        for region in &changed {
            for y in region.top..region.bottom {
                for x in region.left..region.right {
                    let at = (y * size.0 + x) as usize;
                    words[at] = newer[at];
                }
            }
        }
        let written = held.write(size, &words, Some(&changed));
        assert_eq!(written, (10 * 5 + 11 * 7) * 4, "the regions' bytes alone");
        assert_eq!(held.read_back(), Ok(words.clone()), "every word kept");

        assert_eq!(held.write(size, &words, Some(&[])), 0, "nothing changed");
        let wider = (102, 37);
        assert_eq!(
            held.write(wider, &varied(wider, 3), Some(&changed)),
            102 * 37 * 4,
            "another size is written whole"
        );
        assert_eq!(held.read_back(), Ok(varied(wider, 3)));
    }
}
