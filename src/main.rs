mod display;

use std::error::Error;
use std::sync::Arc;
use display::{
    console::Console,
    console_listenner::ConsoleListenerHandler,
    utils::{ WindowCommand},
    pixels_window::build_pixels_window
};
use tokio::sync::{ mpsc::{self, Sender, Receiver}};
use std::os::unix::io::AsRawFd;
use std::os::fd::FromRawFd;



#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Create the channel to manage the communication with the console
    let (sender, mut receiver): (Sender<WindowCommand>, Receiver<WindowCommand>) = mpsc::channel(100);

    // Create the console
    let console_handler = Arc::new(Console::new(0).await);

    // Using minifb
    // build_minifb_window(sender, receiver, console_handler).await;

    // Using Pixels
    build_pixels_window(sender, receiver, console_handler).await;

    Ok(())
}






