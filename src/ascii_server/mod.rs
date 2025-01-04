use std::{
    collections::{hash_map::Entry, HashMap},
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use anyhow::Context;
use futures::{SinkExt, StreamExt};
use nom::Finish;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{debug, info, trace, warn};
use user_manager::UserManager;

use crate::app_state::AppState;
use parser::{parse_request, Request, Response};

mod parser;
mod user_manager;

const MAX_INPUT_LINE_LENGTH: usize = 1024;
const MAX_CONNECTIONS_PER_IP: usize = 2;

const HELP_TEXT: &str = "Help text here :)";

pub struct AsciiServer {
    listener: TcpListener,

    shared_state: Arc<AppState>,
    user_manager: UserManager,
    connections_per_ip: Arc<RwLock<HashMap<IpAddr, usize>>>,

    width: u16,
    height: u16,
}

impl AsciiServer {
    pub async fn new(
        shared_state: Arc<AppState>,
        listener_address: &str,
        width: u16,
        height: u16,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(listener_address).await.with_context(|| {
            format!("Failed to bind to ASCII listener address {listener_address}")
        })?;

        Ok(Self {
            shared_state,
            user_manager: UserManager::new_from_save_file()
                .await
                .context("Failed to create user manager")?,
            connections_per_ip: Default::default(),
            listener,
            width,
            height,
        })
    }

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

    async fn handle_connection(
        &self,
        mut socket: TcpStream,
        peer_addr: SocketAddr,
    ) -> anyhow::Result<()> {
        debug!(%peer_addr, "Got new connection");

        // Check if connection limit is reached
        {
            let mut connections_per_ip = self.connections_per_ip.write().await;
            let connections = connections_per_ip.entry(peer_addr.ip()).or_default();
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
                socket.flush().await.context("Failed to flush socket")?;
                socket
                    .shutdown()
                    .await
                    .context("Failed to shutdown socket")?;

                return Ok(());
            }

            *connections += 1;
        }

        let mut framed = Framed::new(
            socket,
            LinesCodec::new_with_max_length(MAX_INPUT_LINE_LENGTH),
        );

        let mut current_username = None;
        while let Some(line) = framed.next().await {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            let request = Self::parse_request_report_errors(&line, &mut framed).await?;
            if let Some(request) = request {
                let response = self
                    .process_request(request, &mut current_username)
                    .await
                    .context("Failed to process request")?;
                if let Some(response) = response {
                    self.send_response(response, &mut framed, peer_addr.ip())
                        .await
                        .context("Failed to send response to client")?;
                }
            }
        }

        self.dec_connections(peer_addr.ip()).await;
        debug!(%peer_addr, "Connection closed");

        Ok(())
    }

    async fn parse_request_report_errors<'a>(
        line: &'a str,
        framed: &mut Framed<TcpStream, LinesCodec>,
    ) -> anyhow::Result<Option<Request<'a>>> {
        match parse_request(line).finish() {
            Ok(("", request)) => {
                trace!(?request, "Got request");

                Ok(Some(request))
            }
            Ok((remaining, request)) => {
                framed
                    .send(format!("ERROR The request {line:?} could be parsed to {request:?}, but it had remaining bytes: {remaining:?}"))
                    .await
                    .context("Failed to send response to client")?;

                Ok(None)
            }
            Err(err) => {
                framed
                    .send(format!("ERROR Invalid request {line:?}: {err:?}"))
                    .await
                    .context("Failed to send response to client")?;

                Ok(None)
            }
        }
    }

    async fn process_request<'a>(
        &self,
        request: Request<'a>,
        current_username: &mut Option<String>,
    ) -> anyhow::Result<Option<Response>> {
        Ok(match request {
            Request::Help => Some(Response::Help),
            Request::Size => Some(Response::Size {
                width: self.width,
                height: self.height,
            }),
            Request::Login { username, password } => Some({
                if self
                    .user_manager
                    .check_credentials(username, password)
                    .await
                    .context(format!("Failed to check credentials of user {username}"))?
                {
                    *current_username = Some(username.to_owned());
                    Response::LoginSucceeded
                } else {
                    *current_username = None;
                    Response::LoginFailed
                }
            }),
            Request::GetPixel { x, y } => self
                .shared_state
                .framebuffer
                .read()
                .await
                .get(x, y)
                .map(|rgba| Response::GetPixel { x, y, rgba }),
            Request::SetPixel { x, y, rgba } => match current_username {
                Some(current_username) => {
                    // TODO Check rate limit and stuff
                    let client_update = self
                        .shared_state
                        .framebuffer
                        .write()
                        .await
                        .set_client_update(x, y, rgba, current_username.to_owned());

                    if let Some(client_update) = client_update {
                        self.shared_state
                            .web_socket_message_tx
                            .send(client_update)
                            .context("Failed to send update to client")?;
                    }

                    None
                }
                None => Some(Response::LoginNeeded),
            },
        })
    }

    pub async fn send_response(
        &self,
        response: Response,
        framed: &mut Framed<TcpStream, LinesCodec>,
        peer_ip: IpAddr,
    ) -> anyhow::Result<()> {
        let mut close_connection = false;

        match response {
            Response::Help => framed.send(HELP_TEXT).await,
            Response::Size { width, height } => framed.send(format!("SIZE {width} {height}")).await,
            Response::LoginNeeded => {
                close_connection = true;
                framed.send("ERROR LOGIN NEEDED").await
            }
            Response::LoginSucceeded => framed.send("LOGIN SUCCEEDED").await,
            Response::LoginFailed => {
                close_connection = true;
                framed.send("ERROR LOGIN FAILED").await
            }
            Response::GetPixel { x, y, rgba } => {
                framed.send(format!("PX {x} {y} {rgba:06x}")).await
            }
        }
        .context("Failed to send response to client")?;

        if close_connection {
            self.dec_connections(peer_ip).await;

            <Framed<tokio::net::TcpStream, LinesCodec> as SinkExt<String>>::flush(framed)
                .await
                .context("Failed to flush stream")?;
            <Framed<tokio::net::TcpStream, LinesCodec> as SinkExt<String>>::close(framed)
                .await
                .context("Failed to close stream")?;
        }

        Ok(())
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
