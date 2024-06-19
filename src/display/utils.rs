#[cfg(unix)]
use std::os::unix::{io::AsRawFd, net::UnixStream};
use zbus::zvariant::Fd;


pub fn prepare_uds_pass(us: &UnixStream) -> Result<Fd, Box<dyn std::error::Error + Send + Sync>> {
    Ok(us.as_raw_fd().into())
}