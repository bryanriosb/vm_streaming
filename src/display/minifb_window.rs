use std::error::Error;
use std::sync::Arc;
use minifb::{Key, Window, WindowOptions};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::display::console::Console;
use crate::display::console_handler::DisplayHandlers;
use crate::display::utils::WindowCommand;

async fn build_minifb_window(
    sender: Sender<WindowCommand>,
    mut receiver: Receiver<WindowCommand>,
    console_handler: Arc<Result<Console, Box<dyn Error + Send + Sync>>>
) {
    let (thread_sender, mut thread_receiver): (Sender<WindowCommand>, Receiver<WindowCommand>) = mpsc::channel(100);

    let window_thread = std::thread::spawn(move || {
        let mut window = match Window::new(
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