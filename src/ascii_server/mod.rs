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

use crate::app_state::AppState;
use parser::{parse_request, Request, Response};

mod parser;

const MAX_INPUT_LINE_LENGTH: usize = 1024;
const MAX_CONNECTIONS_PER_IP: usize = 2;

pub struct AsciiServer {
    listener: TcpListener,

    _shared_state: Arc<AppState>,
    connections_per_ip: Arc<RwLock<HashMap<IpAddr, usize>>>,

    width: u32,
    height: u32,
}

impl AsciiServer {
    pub async fn new(
        shared_state: Arc<AppState>,
        listener_address: &str,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(listener_address).await.with_context(|| {
            format!("Failed to bind to ASCII listener address {listener_address}")
        })?;

        Ok(Self {
            _shared_state: shared_state,
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

        while let Some(line) = framed.next().await {
            let line = line?;
            let request = Self::parse_request_report_errors(line, &mut framed).await?;
            if let Some(request) = request {
                let response = self.process_request(request).await;
                response
                    .send_response(&mut framed)
                    .await
                    .context("Failed to send response to client")?;
            }
        }

        {
            let mut connections_per_ip = self.connections_per_ip.write().await;
            let connections = connections_per_ip.entry(peer_addr.ip());
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
                    ?peer_addr,
                    ip = ?peer_addr.ip(),
                    "Tried to decrement the number of connections, but this IP had no connection number stored"
                ),
            }
        }
        debug!(%peer_addr, "Connection closed");

        Ok(())
    }

    async fn parse_request_report_errors(
        line: String,
        framed: &mut Framed<TcpStream, LinesCodec>,
    ) -> anyhow::Result<Option<Request>> {
        match parse_request(&line).finish() {
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

    async fn process_request(&self, request: Request) -> Response {
        match request {
            Request::Help => Response::Help,
            Request::Size => Response::Size {
                width: self.width,
                height: self.height,
            },
        }
    }
}
