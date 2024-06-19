mod display;

use std::error::Error;
use std::sync::Arc;
use async_trait::async_trait;
use image::{ImageBuffer, Rgba, RgbaImage};
use minifb::{Key, MouseMode, Window as MinifbWindow, WindowOptions};
use display::console::Console;
use crate::display::console_listenner::{ConsoleListenerHandler, Cursor, MouseSet, Scanout, ScanoutDMABUF, Update, UpdateDMABUF};
use tokio::sync::{ mpsc::{self, Sender, Receiver}};
use libc::{mmap, munmap, MAP_SHARED, PROT_READ, PROT_WRITE};
use std::os::unix::io::AsRawFd;
use std::os::fd::FromRawFd;
use std::time::{Duration, Instant};


// Pixels window
use pixels::{Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;




#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (sender, mut receiver): (Sender<WindowCommand>, Receiver<WindowCommand>) = mpsc::channel(100);

    let console_handler = Arc::new(Console::new(0).await);

    // Using minifb
    // build_minifb_window(sender, receiver, console_handler).await;

    // Using Pixels
    build_pixels_window(sender, receiver, console_handler).await;

    Ok(())
}

#[derive()]
enum WindowCommand {
    Update(Vec<u32>, usize, usize),
    UpdateDMABUF(Vec<u8>),
    MouseMove(f32, f32), // x, y
    KeyPress(Key),
}

struct DisplayHandlers {
    sender: Sender<WindowCommand>,
}

impl DisplayHandlers {
    fn new(sender: Sender<WindowCommand>) -> Self {
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

async fn build_pixels_window(
    sender: Sender<WindowCommand>,
    mut receiver: Receiver<WindowCommand>,
    console_handler: Arc<Result<Console, Box<dyn Error + Send + Sync>>>
) {
    // Create an event loop
    let event_loop = EventLoop::new();

    // Create a window
    let window = WindowBuilder::new()
        .with_title("Pixels Example")
        .with_inner_size(LogicalSize::new(1280, 800))
        .build(&event_loop)
        .unwrap();

    // Create a surface texture
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(
        window_size.width, window_size.height, &window
    );

    // Create a Pixels instance
    let mut pixels = Pixels::new(1280, 800, surface_texture).unwrap();

    // Connect to the console and register the DBus listener
    let sender_clone = sender.clone();
    let cloned_console_handler_2 = Arc::clone(&console_handler);
    tokio::spawn(async move {
        match cloned_console_handler_2.as_ref() {
            Ok(console) => {
                println!("Connected to console");
                let handlers = DisplayHandlers::new(sender_clone);
                let _ = console.register_listener(handlers).await.unwrap();
            }
            Err(e) => {
                panic!("Error: {}", e);
            }
        }
    });

    // Main event loop timer
    let mut last_update = Instant::now();

    // Event loop to keep the window open and render the pixels
    event_loop.run(move |event, _, control_flow| {
        println!("Received event: {:?}", event);
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => {
                    let _ = pixels.resize_surface(size.width, size.height);
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                if pixels.render().is_err() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }
            _ => (),
        }

        if let Ok(command) = receiver.try_recv() {
            match command {
                WindowCommand::Update(buffer, width, height) => { },
                WindowCommand::UpdateDMABUF(buffer) => {
                    if last_update.elapsed() >= Duration::from_millis(16) { // ~60 FPS
                        update_frame_from_dmabuf(pixels.frame_mut(), &buffer);
                        window.request_redraw();
                        last_update = Instant::now();
                    }
                }
                _ => {}
            }
        }

        window.request_redraw();
    });
}

fn update_frame_from_dmabuf(frame: &mut [u8], buffer: &[u8]) {
    println!("Updating the pixels frame from DMABUF");
    // Manipulate the pixel buffer
    // for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
    //     let x = (i % 1280) as u8;
    //     let y = (i / 800) as u8;
    //     pixel[0] = x; // R
    //     pixel[1] = y; // G
    //     pixel[2] = 0; // B
    //     pixel[3] = 255; // A
    // }

    frame.copy_from_slice(&buffer);
}

async fn build_minifb_window(
    sender: Sender<WindowCommand>,
    mut receiver: Receiver<WindowCommand>,
    console_handler: Arc<Result<Console, Box<dyn Error + Send + Sync>>>
) {
    let (thread_sender, mut thread_receiver): (Sender<WindowCommand>, Receiver<WindowCommand>) = mpsc::channel(100);

    let window_thread = std::thread::spawn(move || {
        let mut window = match MinifbWindow::new(
            "Qemu Display Buffer",
            1280,
            800,
            {
                let mut options = WindowOptions::default();
                options.resize = true;
                options
            }
        ) {
            Ok(win) => win,
            Err(e) => {
                eprintln!("Failed to create window: {}", e);
                std::process::exit(1);
            }
        };

        // let target_fps = 60.0;
        // let frame_duration = std::time::Duration::from_secs_f64(1.0 / target_fps);

        while window.is_open() && !window.is_key_down(Key::Escape) {
            // let frame_start = std::time::Instant::now();

            // let mouse_pos = window.get_mouse_pos(MouseMode::Clamp);
            //
            // if let Some((x, y)) = mouse_pos {
            //     let _ = thread_sender.blocking_send(WindowCommand::MouseMove(x, y)).unwrap();
            // }

            // Recibir comando del canal en el hilo principal
            if let Ok(command) = receiver.try_recv() {
                match command {
                    WindowCommand::Update(buffer, width, height) => {
                        window.update_with_buffer(&buffer, width, height).unwrap();
                    },
                    _ => {}
                }
            }

            // Wait for the next frame
            // let elapsed = frame_start.elapsed();
            // if elapsed < frame_duration {
            //     std::thread::sleep(frame_duration - elapsed);
            // }
        }
    });

    // Connect to the console and register the DBus listener
    let sender_clone = sender.clone();
    let cloned_console_handler_2 = Arc::clone(&console_handler);
    tokio::spawn(async move {
        match cloned_console_handler_2.as_ref() {
            Ok(console) => {
                println!("Connected to console");
                let handlers = DisplayHandlers::new(sender_clone);
                let _ = console.register_listener(handlers).await.unwrap();
            }
            Err(e) => {
                panic!("Error: {}", e);
            }
        }
    });

    // Procesar comandos recibidos asincrónicamente
    tokio::spawn({
        async move {
            while let Some(command) = thread_receiver.recv().await {
                match command {
                    WindowCommand::MouseMove(x, y) => {
                        let cloned_console_handler_3 = Arc::clone(&console_handler);
                        tokio::spawn(async move {
                            println!("Received mouse event: ({}, {})", x, y);
                            match cloned_console_handler_3.as_ref(){
                                Ok(guard) => {
                                    guard.mouse.set_abs_position(x as u32, y as u32).await.unwrap();
                                }
                                Err(e) => {
                                    println!("No console found {:?}", e);
                                }
                            }
                            // update_mouse_position(cloned_console_handler_3, x, y).await
                        });
                    }
                    _ => {}
                }
            }
        }
    });

    // Esperar a que el hilo de la ventana termine
    window_thread.join().unwrap();
}

// async fn update_mouse_position(console:Arc<Mutex<Result<Console, Box<dyn Error + Send + Sync>>>>, x: f32, y: f32) {
//     println!("Entering update_mouse_position: ({}, {})", x, y);
//     match console.lock().await.as_mut() {
//         Ok(guard) => {
//             println!("Guard: {:?}", guard);
//
//             guard.mouse.set_abs_position(x as u32, y as u32).await.unwrap();
//         }
//         Err(e) => {
//             println!("No console found {:?}", e);
//         }
//     }
// }
