use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use pixels::{Pixels, SurfaceTexture};
use tokio::sync::mpsc::{Receiver, Sender};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use crate::display::console::Console;
use crate::display::console_handler::DisplayHandlers;
use crate::display::utils::WindowCommand;

pub async fn build_pixels_window(
    sender: Sender<WindowCommand>,
    mut receiver: Receiver<WindowCommand>,
    console_handler: Arc<Result<Console, Box<dyn Error + Send + Sync>>>
) {
    let window_width = 400;
    let window_height = 300 ad;

    // Create an event loop
    let event_loop = EventLoop::new();

    // Create a window
    let window = WindowBuilder::new()
        .with_title("Pixels Example")
        .with_inner_size(LogicalSize::new(window_width, window_height))
        .build(&event_loop)
        .unwrap();

    // Create a surface texture
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(
        window_size.width, window_size.height, &window
    );

    // Create a Pixels instance
    let mut pixels = Pixels::new(window_width, window_height, surface_texture).unwrap();

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
    //     let x = (i % 340) as u8;
    //     let y = (i / 220) as u8;
    //     pixel[0] = x; // R
    //     pixel[1] = y; // G
    //     pixel[2] = 0; // B
    //     pixel[3] = 255; // A
    // }

    frame.copy_from_slice(&buffer);
}