use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use futures::{SinkExt, StreamExt};
use nom::Finish;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{debug, info, trace};

use crate::app_state::AppState;
use parser::{parse_request, Request, Response};

mod parser;

const MAX_INPUT_LINE_LENGTH: usize = 1024;

pub struct AsciiServer {
    _shared_state: Arc<AppState>,
    listener: TcpListener,
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
        socket: TcpStream,
        peer_addr: SocketAddr,
    ) -> anyhow::Result<()> {
        debug!(%peer_addr, "Got new connection");

        let mut framed = Framed::new(
            socket,
            LinesCodec::new_with_max_length(MAX_INPUT_LINE_LENGTH),
        );

        while let Some(line) = framed.next().await {
            let line = line?;
            let request = Self::parse_request_report_errors(line, &mut framed).await?;
            if let Some(request) = request {
                let response = self.process_request(request);
                response
                    .send_response(&mut framed)
                    .await
                    .context("Failed to send response to client")?;
            }
        }

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
                    .send(format!("ERROR: The request {line:?} could be parsed to {request:?}, but it had remaining bytes: {remaining:?}"))
                    .await
                    .context("Failed to send response to client")?;

                Ok(None)
            }
            Err(err) => {
                framed
                    .send(format!("ERROR: Invalid request {line:?}: {err:?}"))
                    .await
                    .context("Failed to send response to client")?;

                Ok(None)
            }
        }
    }

    fn process_request(&self, request: Request) -> Response {
        match request {
            Request::Help => Response::Help,
            Request::Size => Response::Size {
                width: self.width,
                height: self.height,
            },
        }
    }
}
