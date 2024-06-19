#[cfg(unix)]
use std::os::unix::{io::AsRawFd, net::UnixStream};
use minifb::Key;
use zbus::zvariant::Fd;


pub fn prepare_uds_pass(us: &UnixStream) -> Result<Fd, Box<dyn std::error::Error + Send + Sync>> {
    Ok(us.as_raw_fd().into())
}

pub enum WindowCommand {
    Update(Vec<u32>, usize, usize),
    UpdateDMABUF(Vec<u8>),
    MouseMove(f32, f32), // x, y
    KeyPress(Key),
}