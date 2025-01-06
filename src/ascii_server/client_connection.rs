use anyhow::Context;
use futures::{SinkExt, StreamExt};
use nom::Finish;
use tokio::{net::TcpStream, sync::mpsc};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::trace;

use crate::app_state::AppState;

use super::{
    parser::{parse_request, Request, Response},
    user_manager::UserManager,
    HELP_TEXT, MAX_INPUT_LINE_LENGTH,
};

pub struct ClientConnection<'a> {
    user_manager: &'a UserManager,
    shared_state: &'a AppState,

    _slot_start: mpsc::Receiver<()>,
    _slot_end: mpsc::Receiver<()>,

    width: u16,
    height: u16,
}

impl<'a> ClientConnection<'a> {
    pub fn new(
        user_manager: &'a UserManager,
        shared_state: &'a AppState,
        slot_start: mpsc::Receiver<()>,
        slot_end: mpsc::Receiver<()>,
        width: u16,
        height: u16,
    ) -> Self {
        Self {
            user_manager,
            shared_state,
            _slot_start: slot_start,
            _slot_end: slot_end,
            width,
            height,
        }
    }

    pub async fn run(&self, socket: &mut TcpStream) -> anyhow::Result<()> {
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
                    let close_connection = self
                        .send_response(response, &mut framed)
                        .await
                        .context("Failed to send response to client")?;
                    if close_connection {
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    async fn parse_request_report_errors<'line>(
        line: &'line str,
        framed: &mut Framed<&mut TcpStream, LinesCodec>,
    ) -> anyhow::Result<Option<Request<'line>>> {
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

    async fn process_request<'request>(
        &self,
        request: Request<'request>,
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
                            .ws_message_tx
                            .send(client_update)
                            .await
                            .context("Failed to send update to websocket message channel")?;
                    }

                    None
                }
                None => Some(Response::LoginNeeded),
            },
        })
    }

    /// Sends the given response to the client and returns if the connection should be closed
    pub async fn send_response(
        &self,
        response: Response,
        framed: &mut Framed<&mut TcpStream, LinesCodec>,
    ) -> anyhow::Result<bool> {
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

        Ok(close_connection)
    }
}
