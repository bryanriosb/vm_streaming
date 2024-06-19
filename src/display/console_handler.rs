use async_trait::async_trait;
use image::{ImageBuffer, RgbaImage};
use libc::{MAP_SHARED, mmap, munmap, PROT_READ};
use tokio::sync::mpsc::Sender;
use crate::display::console_listenner::{ConsoleListenerHandler, Cursor, MouseSet, Scanout, ScanoutDMABUF, Update, UpdateDMABUF};
use crate::display::utils::WindowCommand;
use std::os::unix::io::AsRawFd;

pub struct DisplayHandlers {
    sender: Sender<WindowCommand>,
}

impl DisplayHandlers {
    pub fn new(sender: Sender<WindowCommand>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl ConsoleListenerHandler for DisplayHandlers {
    async fn scanout(&mut self, scanout: Scanout) {
        println!("Scanout received: {:?}", scanout);

        let img = scanout.to_image();

        // Convert the image to a Vec<u32> for minifb
        let buffer: Vec<u32> = img
            .pixels()
            .map(|p| {
                let channels = p.0;
                (u32::from(channels[2]) << 16) // Red
                    | (u32::from(channels[1]) << 8)  // Green
                    | u32::from(channels[0])         // Blue
                    | (u32::from(channels[3]) << 24) // Alpha, if needed
            })
            .collect();

        let vec_buffer = buffer.to_vec();

        // Lock the window and update it with the buffer
        self.sender.send(
            WindowCommand::Update(
                buffer,
                scanout.width as usize,
                scanout.height as usize
            )
        ).await.unwrap();
    }

    async fn update(&mut self, update: Update) {
        println!("Update received: {:?}", update);
    }

    #[cfg(unix)]
    async fn scanout_dmabuf(&mut self, scanout: ScanoutDMABUF) {
        println!("Scanout DMABUF received: {:?}", scanout);
        let buffer = scanout.map_dmabuf_to_buffer();

        self.sender.send(
            WindowCommand::UpdateDMABUF(
                buffer
            )).await.unwrap();

    }

    #[cfg(unix)]
    async fn update_dmabuf(&mut self, update: UpdateDMABUF) {
        println!("Update DMABUF received: {:?}", update);
    }

    async fn mouse_set(&mut self, set: MouseSet) {
        println!("MouseSet received: {:?}", set);
    }

    async fn cursor_define(&mut self, cursor: Cursor) {
        println!("Cursor received: {:?}", cursor);
    }

    fn disconnected(&mut self) {
        println!("Disconnected");
    }
}


impl ScanoutDMABUF {
    fn map_dmabuf_to_buffer(&self) -> Vec<u8>  {
        let height = self.height;
        let stride = self.stride;

        let buffer = unsafe {
            let size = (height  * stride) as usize;

            let ptr = mmap(
                std::ptr::null_mut(),
                size,
                PROT_READ,
                MAP_SHARED,
                self.fd.as_raw_fd(),
                0,
            );

            if ptr == libc::MAP_FAILED {
                panic!("Failed to mmap DMABUF");
            }

            let slice = std::slice::from_raw_parts(ptr as *mut u8, size);
            let mut buffer: Vec<u8> = slice.to_vec();

            munmap(ptr, size);
            buffer
        };

        buffer
    }

    fn to_pixel_buffer(&self, buffer: &[u8]) -> Vec<u32> {
        let width = self.width;
        let height = self.height;
        let stride = self.stride;
        let size = (height * stride) as usize;

        // Buffer de píxeles para minifb
        let mut pixel_buffer: Vec<u32> = vec![0u32; (width * height) as usize];
        //let mut pixel_buffer = ImageBuffer::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let offset = (y * stride + x * 4) as usize;
                if offset + 3 < size {
                    let pixel = (u32::from(buffer[offset + 3]) << 24) // Alpha
                        | (u32::from(buffer[offset]) << 16)         // Red
                        | (u32::from(buffer[offset + 1]) << 8)      // Green
                        | u32::from(buffer[offset + 2]);            // Blue
                    pixel_buffer[(y * width + x) as usize] = pixel;
                }
            }
        }

        // let pixel_buffer: Vec<u32> = pixel_buffer
        //     .pixels()
        //     .map(|p| {
        //         let channels = p.0;
        //         (u32::from(channels[2]) << 16) // Red
        //             | (u32::from(channels[1]) << 8)  // Green
        //             | u32::from(channels[0])         // Blue
        //             | (u32::from(channels[3]) << 24) // Alpha, if needed
        //     })
        //     .collect();

        pixel_buffer
    }
}


impl Scanout {
    fn to_image(&self) -> RgbaImage {
        let width = self.width as u32;
        let height = self.height as u32;
        let stride = self.stride as usize;

        let mut img = ImageBuffer::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let index = (y as usize * stride) + (x as usize * 4); // Assuming 4 bytes per pixel (RGBA)
                let pixel = [
                    self.data[index],
                    self.data[index + 1],
                    self.data[index + 2],
                    self.data[index + 3],
                ];
                img.put_pixel(x, y, image::Rgba(pixel));
            }
        }

        img
    }
}