use std::convert::TryFrom;
use std::sync::Arc;
use vulkano::command_buffer::PrimaryAutoCommandBuffer;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::Device;
use vulkano::device::DeviceExtensions;
use vulkano::image::view::ImageView;
use vulkano::image::AttachmentImage;
use vulkano::image::ImageAccess;
use vulkano::image::ImageUsage;
use vulkano::image::SwapchainImage;
use vulkano::instance::Instance;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::Framebuffer;
use vulkano::render_pass::RenderPass;
use vulkano::swapchain::{self, AcquireError, Swapchain, SwapchainCreationError};
use vulkano::sync::{self, GpuFuture};
use winit::event_loop::EventLoop;
use winit::window::Window;
use winit::window::WindowBuilder;

pub struct Vulkan {
    pub surface: Arc<vulkano::swapchain::Surface<winit::window::Window>>,
    pub device: Arc<vulkano::device::Device>,
    pub present_mode: vulkano::swapchain::PresentMode,
    pub min_image_count: u32,
    pub queue: Arc<vulkano::device::Queue>,
    pub render_pass: Arc<vulkano::render_pass::RenderPass>,
    pub swapchain: Arc<Swapchain<winit::window::Window>>,
    pub viewport: Viewport,
    pub framebuffers: Vec<Arc<vulkano::render_pass::Framebuffer>>,
    pub recreate_swapchain: bool,
    pub previous_frame_end: Option<Box<dyn vulkano::sync::GpuFuture>>,
}

impl Vulkan {
    pub fn new(wb: WindowBuilder, event_loop: &EventLoop<()>) -> Self {
        dbg!(vulkano::Version::HEADER_VERSION);
        let required_extensions = vulkano::instance::InstanceExtensions {
            ext_debug_report: true,
            ..vulkano_win::required_extensions()
        };
        let instance = Instance::new(vulkano::instance::InstanceCreateInfo {
            enabled_extensions: required_extensions,
            enabled_layers: vec!["VK_LAYER_KHRONOS_validation".to_string()],
            max_api_version: Some(vulkano::Version::V1_2),
            ..Default::default()
        })
        .unwrap();

        use vulkano::instance::debug::{DebugCallback, MessageSeverity, MessageType};
        let _callback = DebugCallback::new(
            &instance,
            MessageSeverity::all(),
            MessageType::all(),
            |msg| {
                println!("Debug callback: {:?}", msg.description);
            },
        )
        .ok();

        use vulkano_win::VkSurfaceBuild;
        let surface = wb.build_vk_surface(event_loop, instance.clone()).unwrap();
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };
        let (physical_device, queue_family) = PhysicalDevice::enumerate(&instance)
            .filter(|&p| p.supported_extensions().is_superset_of(&device_extensions))
            .filter_map(|p| {
                p.queue_families()
                    .find(|&q| {
                        q.supports_graphics() && q.supports_surface(&surface).unwrap_or(false)
                    })
                    .map(|q| (p, q))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
            })
            .unwrap();
        let (device, mut queues) = Device::new(
            physical_device,
            vulkano::device::DeviceCreateInfo {
                enabled_extensions: physical_device
                    .required_extensions()
                    .union(&device_extensions),
                queue_create_infos: vec![vulkano::device::QueueCreateInfo::family(queue_family)],
                ..Default::default()
            },
        )
        .unwrap();
        let present_mode = Self::best_present_mode(&physical_device, &surface);
        let caps = physical_device
            .surface_capabilities(&surface, vulkano::swapchain::SurfaceInfo::default())
            .unwrap();
        let min_image_count = caps.min_image_count + 1;
        let queue = queues.next().unwrap();
        let (swapchain, images) = {
            let dimensions: [u32; 2] = surface.window().inner_size().into();
            Swapchain::new(
                device.clone(),
                surface.clone(),
                vulkano::swapchain::SwapchainCreateInfo {
                    image_extent: dimensions,
                    image_usage: ImageUsage::color_attachment(),
                    min_image_count,
                    present_mode,
                    ..Default::default()
                },
            )
            .unwrap()
        };
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.image_format(),
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: vulkano::format::Format::D32_SFLOAT,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {depth}
            }
        )
        .unwrap();

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };

        let framebuffers = Self::window_size_dependent_setup(
            device.clone(),
            &images,
            render_pass.clone(),
            &mut viewport,
        );
        let recreate_swapchain = false;
        let previous_frame_end = Some(sync::now(device.clone()).boxed());

        Self {
            surface,
            device,
            present_mode,
            min_image_count,
            render_pass,
            queue,
            swapchain,
            viewport,
            framebuffers,
            recreate_swapchain,
            previous_frame_end,
        }
    }
    fn window_size_dependent_setup(
        device: Arc<Device>,
        images: &[Arc<SwapchainImage<Window>>],
        render_pass: Arc<RenderPass>,
        viewport: &mut Viewport,
    ) -> Vec<Arc<Framebuffer>> {
        let dimensions = images[0].dimensions().width_height();
        viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];
        images
            .iter()
            .map(|image| {
                let view = ImageView::new_default(image.clone()).unwrap();
                let depth_buffer = ImageView::new_default(
                    AttachmentImage::with_usage(
                        device.clone(),
                        dimensions,
                        vulkano::format::Format::D32_SFLOAT,
                        ImageUsage {
                            depth_stencil_attachment: true,
                            transient_attachment: true,
                            ..ImageUsage::none()
                        },
                    )
                    .unwrap(),
                )
                .unwrap();

                Framebuffer::new(
                    render_pass.clone(),
                    vulkano::render_pass::FramebufferCreateInfo {
                        attachments: vec![view, depth_buffer],
                        ..Default::default()
                    },
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
    }

    pub fn recreate_swapchain_if_necessary(&mut self) {
        {
            if let Some(mut fut) = self.previous_frame_end.take() {
                fut.cleanup_finished();
                // We need to synchronize here to send new data to the GPU.
                // We can't send the new framebuffer until the previous frame is done being drawn.
                // Dropping the future will block until it's done.
            }
        }
        if self.recreate_swapchain {
            let dimensions: [u32; 2] = self.surface.window().inner_size().into();
            let (new_swapchain, new_images) =
                match self
                    .swapchain
                    .recreate(vulkano::swapchain::SwapchainCreateInfo {
                        image_extent: dimensions,
                        image_usage: ImageUsage::color_attachment(),
                        present_mode: self.present_mode,
                        min_image_count: self.min_image_count,
                        ..Default::default()
                    }) {
                    Ok(r) => r,
                    Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                    Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                };

            self.swapchain = new_swapchain;
            self.framebuffers = Self::window_size_dependent_setup(
                self.device.clone(),
                &new_images,
                self.render_pass.clone(),
                &mut self.viewport,
            );
            self.recreate_swapchain = false;
        }
    }

    fn best_present_mode<W>(
        dev: &vulkano::device::physical::PhysicalDevice,
        surf: &vulkano::swapchain::Surface<W>,
    ) -> vulkano::swapchain::PresentMode {
        dev.surface_present_modes(surf)
            .unwrap()
            .find(|m| [vulkano::swapchain::PresentMode::Mailbox].contains(m))
            .unwrap_or(vulkano::swapchain::PresentMode::Fifo)
    }

    pub fn get_next_image(&mut self) -> Option<usize> {
        let (image_num, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return None;
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };
        if suboptimal {
            self.recreate_swapchain = true;
        }
        let old_fut = self.previous_frame_end.take();
        self.previous_frame_end = match old_fut {
            None => Some(Box::new(acquire_future)),
            Some(old_fut) => Some(Box::new(old_fut.join(acquire_future))),
        };
        Some(image_num)
    }
    pub fn execute_commands(&mut self, command_buffer: PrimaryAutoCommandBuffer, image_num: usize) {
        let old_fut = self.previous_frame_end.take();
        let future = old_fut
            .unwrap_or_else(|| vulkano::sync::now(self.device.clone()).boxed())
            .then_execute(self.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), image_num)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                self.previous_frame_end = Some(future.boxed());
            }
            Err(vulkano::sync::FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                self.previous_frame_end = Some(vulkano::sync::now(self.device.clone()).boxed());
            }
            Err(e) => {
                println!("Failed to flush future: {:?}", e);
                self.previous_frame_end = Some(vulkano::sync::now(self.device.clone()).boxed());
            }
        }
    }
    pub fn wait_for(&mut self, f: Box<dyn GpuFuture>) {
        let old_fut = self.previous_frame_end.take();
        self.previous_frame_end = match old_fut {
            None => Some(f),
            Some(old_fut) => Some(Box::new(old_fut.join(f))),
        };
    }
}
