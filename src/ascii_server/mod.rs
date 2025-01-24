use std::{
    collections::{hash_map::Entry, HashMap},
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use client_connection::ClientConnection;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tracing::{debug, info, warn};
use user_scheduler::UserScheduler;

use crate::{app_state::AppState, ascii_server::user_manager::UserManager};

mod client_connection;
mod parser;
mod user_manager;
mod user_scheduler;

const MAX_INPUT_LINE_LENGTH: usize = 128;
const MAX_CONNECTIONS_PER_IP: usize = 2;

const HELP_TEXT: &str = "Help text here :)";

pub struct AsciiServer<'a> {
    listener: TcpListener,

    shared_state: Arc<AppState>,
    user_manager: UserManager,
    user_scheduler: Arc<UserScheduler>,
    connections_per_ip: Arc<RwLock<HashMap<IpAddr, usize>>>,

    _client_connections: HashMap<&'a str, ClientConnection<'a>>,

    max_pixels_per_slot: usize,
    slot_duration: Duration,

    width: u16,
    height: u16,
}

impl AsciiServer<'_> {
    pub async fn new(
        shared_state: Arc<AppState>,
        listener_address: &str,
        max_pixels_per_slot: usize,
        slot_duration: Duration,
        width: u16,
        height: u16,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(listener_address).await.with_context(|| {
            format!("Failed to bind to ASCII listener address {listener_address}")
        })?;

        let user_scheduler = Arc::new(UserScheduler::new(slot_duration));

        let user_scheduler_clone = user_scheduler.clone();
        tokio::spawn(async move {
            user_scheduler_clone.run().await;
        });

        Ok(Self {
            shared_state,
            user_manager: UserManager::new_from_save_file()
                .await
                .context("Failed to create user manager")?,
            user_scheduler,
            connections_per_ip: Default::default(),
            _client_connections: Default::default(),
            listener,
            max_pixels_per_slot,
            slot_duration,
            width,
            height,
        })
    }

    async fn handle_connection(
        &self,
        mut socket: TcpStream,
        peer_addr: SocketAddr,
    ) -> anyhow::Result<()> {
        // If you connect via IPv4 you often show up as embedded inside an IPv6 address
        // Extracting the embedded information here, so we get the real (TM) address
        let peer_ip = peer_addr.ip().to_canonical();

        debug!(%peer_ip, %peer_addr, "Got new connection");

        if !self
            .check_and_increment_connection_limit(peer_ip, &mut socket)
            .await
            .context("Failed to check and increment connection limit")?
        {
            return Ok(());
        }

        let mut client_connection = ClientConnection::new(
            &self.user_manager,
            &self.user_scheduler,
            &self.shared_state,
            self.max_pixels_per_slot,
            self.slot_duration,
            self.width,
            self.height,
        );
        debug!(%peer_ip, %peer_addr, "Closing connection");
        client_connection
            .run(&mut socket)
            .await
            .context("Failed to run client connection")?;

        socket
            .shutdown()
            .await
            .context("Failed to shut down connection")?;

        self.dec_connections(peer_ip).await;
        debug!(%peer_ip, %peer_addr, "Connection closed");

        Ok(())
    }

    /// Checks if this IP has reached the connection limit, if not increments the connection counter
    async fn check_and_increment_connection_limit(
        &self,
        peer_ip: IpAddr,
        socket: &mut TcpStream,
    ) -> anyhow::Result<bool> {
        let mut connections_per_ip = self.connections_per_ip.write().await;
        let connections = connections_per_ip.entry(peer_ip).or_default();
        if *connections >= MAX_CONNECTIONS_PER_IP {
            socket
                .write_all(
                    format!(
                        "ERROR Connection limit of {MAX_CONNECTIONS_PER_IP} connections per IP reached\n"
                    )
                    .as_bytes(),
                )
                .await
                .context("Failed to send response to client")?;
            socket
                .shutdown()
                .await
                .context("Failed to shutdown socket")?;

            return Ok(false);
        }

        *connections += 1;
        Ok(true)
    }

    async fn dec_connections(&self, ip: IpAddr) {
        let mut connections_per_ip = self.connections_per_ip.write().await;
        let connections = connections_per_ip.entry(ip);
        match connections {
            Entry::Occupied(mut entry) => {
                let value = entry.get_mut();
                if *value <= 1 {
                    entry.remove();
                } else {
                    *value -= 1;
                }
            }
            Entry::Vacant(_) => warn!(
                ?ip,
                "Tried to decrement the number of connections, but this IP had no connection number stored"
            ),
        }
    }
}

impl AsciiServer<'static> {
    pub async fn run(self) -> anyhow::Result<()> {
        info!("Started ASCII server at localhost:1234");
        let server = Arc::new(self);

        loop {
            let (socket, peer_addr) = server
                .listener
                .accept()
                .await
                .context("Failed to accept new TCP connection")?;

            let server_for_loop = server.clone();
            tokio::spawn(async move { server_for_loop.handle_connection(socket, peer_addr).await });
        }
    }
}
