use std::net::SocketAddr;

use anyhow::Context;
use futures::{SinkExt, StreamExt};
use nom::Finish;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{debug, info, trace};

use parser::{parse_request, Request, Response};

mod parser;

const MAX_INPUT_LINE_LENGTH: usize = 1024;

pub async fn run_ascii_server(listener_address: &str) -> anyhow::Result<()> {
    let listener = TcpListener::bind(listener_address)
        .await
        .with_context(|| format!("Failed to bind to ASCII listener address {listener_address}"))?;
    info!("Started ASCII server at localhost:1234");

    loop {
        let (socket, peer_addr) = listener.accept().await?;

        tokio::spawn(async move { handle_connection(socket, peer_addr).await });
    }
}

async fn handle_connection(socket: TcpStream, peer_addr: SocketAddr) -> anyhow::Result<()> {
    debug!(%peer_addr, "Got new connection");

    let mut framed = Framed::new(
        socket,
        LinesCodec::new_with_max_length(MAX_INPUT_LINE_LENGTH),
    );

    while let Some(line) = framed.next().await {
        let line = line?;
        let request = parse_request_report_errors(line, &mut framed).await?;
        if let Some(request) = request {
            let response = process_request(request);
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

fn process_request(request: Request) -> Response {
    match request {
        Request::Help => Response::Help,
        // FIXME
        Request::Size => Response::Size {
            width: 42,
            height: 42,
        },
    }
}
