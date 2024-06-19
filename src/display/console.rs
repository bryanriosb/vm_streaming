use crate::display::utils::prepare_uds_pass;
#[cfg(unix)]
use zbus::zvariant::Fd;
use zbus::{dbus_proxy, zvariant::ObjectPath, Connection};
use std::os::unix::net::UnixStream;
use std::{convert::TryFrom};
use tokio::sync::RwLock;
use crate::display::console_listenner::{ConsoleListener, ConsoleListenerHandler};
use crate::display::keyboard::KeyboardProxy;
use crate::display::mouse::MouseProxy;

#[dbus_proxy(default_service = "org.qemu",  interface = "org.qemu.Display1.Console")]
pub trait Console {
    /// RegisterListener method
    #[dbus_proxy(name = "RegisterListener")]
    fn register_listener(&self, listener: Fd) -> zbus::Result<()>;

    /// SetUIInfo method
    #[dbus_proxy(name = "SetUIInfo")]
    fn set_ui_info(
        &self,
        width_mm: u16,
        height_mm: u16,
        xoff: i32,
        yoff: i32,
        width: u32,
        height: u32,
    ) -> zbus::Result<()>;

    #[dbus_proxy(property)]
    fn label(&self) -> zbus::Result<String>;

    #[dbus_proxy(property)]
    fn head(&self) -> zbus::Result<u32>;

    #[dbus_proxy(property)]
    fn type_(&self) -> zbus::Result<String>;

    #[dbus_proxy(property)]
    fn width(&self) -> zbus::Result<u32>;

    #[dbus_proxy(property)]
    fn height(&self) -> zbus::Result<u32>;
}

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct Console {
    #[derivative(Debug = "ignore")]
    pub proxy: ConsoleProxy<'static>,
    #[derivative(Debug = "ignore")]
    pub keyboard: KeyboardProxy<'static>,
    #[derivative(Debug = "ignore")]
    pub mouse: MouseProxy<'static>,
    listener: RwLock<Option<Connection>>,
}

impl Console {
    pub async fn new(idx: u32) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let obj_path = ObjectPath::try_from(format!("/org/qemu/Display1/Console_{}", idx))?;
        let connection = Connection::session().await?;

        let proxy = ConsoleProxy::builder(&connection).path(&obj_path)?.build().await?;
        let keyboard = KeyboardProxy::builder(&connection)
            .path(&obj_path)?
            .build()
            .await?;
        let mouse = MouseProxy::builder(&connection).path(&obj_path)?.build().await?;

        Ok(Self {
            proxy,
            keyboard,
            mouse,
            listener: RwLock::new(None),
        })
    }

    pub async fn register_listener<H: ConsoleListenerHandler>(&self, handler: H) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Preparing UnixStream pair");
        let (p0, p1) = UnixStream::pair()?;
        let p0 = prepare_uds_pass(&p0)?;

        // println!("Registering listener via proxy")
        self.proxy.register_listener(p0).await?;

        println!("Building D-Bus connection");
        let connection_result = zbus::ConnectionBuilder::unix_stream(p1)
            .p2p()
            .serve_at("/org/qemu/Display1/Listener", ConsoleListener::new(handler))?
            .build()
            .await;

        match connection_result {
            Ok(connection) => {
                println!("Connection built successfully");
                let mut listener_guard = self.listener.write().await;
                *listener_guard = Some(connection);
                println!("Registered listener");
            }
            Err(e) => {
                println!("Failed to build connection: {}", e);
            }
        }
        Ok(())
    }

    pub async fn unregister_listener(&self) {
        let mut listener_guard = self.listener.write().await;
        *listener_guard = None;
    }
}
