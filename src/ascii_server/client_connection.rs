use std::time::Duration;

use anyhow::Context;
use futures::{SinkExt, StreamExt};
use nom::Finish;
use tokio::{net::TcpStream, select, sync::mpsc};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};
use tracing::trace;

use crate::{app_state::AppState, framebuffer::PixelUpdate};

use super::{
    parser::{parse_request, Request, Response},
    user_manager::UserManager,
    HELP_TEXT, MAX_INPUT_LINE_LENGTH,
};

pub struct ClientConnection<'a> {
    user_manager: &'a UserManager,
    shared_state: &'a AppState,

    slot_start: mpsc::Receiver<()>,
    slot_end: mpsc::Receiver<()>,
    max_pixels_per_slot: usize,
    max_slot_duration: Duration,
    painted: Vec<PixelUpdate>,

    width: u16,
    height: u16,

    // State
    current_username: Option<String>,
    currently_in_slot: bool,
    current_pixel_count: usize,
}

impl<'a> ClientConnection<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_manager: &'a UserManager,
        shared_state: &'a AppState,
        slot_start: mpsc::Receiver<()>,
        slot_end: mpsc::Receiver<()>,
        max_pixels_per_slot: usize,
        max_slot_duration: Duration,
        width: u16,
        height: u16,
    ) -> Self {
        Self {
            user_manager,
            shared_state,
            slot_start,
            slot_end,
            max_pixels_per_slot,
            max_slot_duration,
            painted: Default::default(),
            width,
            height,
            current_username: None,
            currently_in_slot: false,
            current_pixel_count: 0,
        }
    }

    pub async fn run(&mut self, socket: &mut TcpStream) -> anyhow::Result<()> {
        let mut framed = Framed::new(
            socket,
            LinesCodec::new_with_max_length(MAX_INPUT_LINE_LENGTH),
        );

        loop {
            enum Next {
                ClientInput(Option<Result<String, LinesCodecError>>),
                SlotStart,
                SlotEnded,
            }

            let next = select! {
                // Cancellation safety: According to [`Framed`], [`tokio_stream::StreamExt::next`] is cancellation safe
                line = framed.next() => Next::ClientInput(line),
                // Cancellation safety: [`tokio::sync::mpsc::Receiver::recv`] is cancellation safe
                _ = self.slot_start.recv() => Next::SlotStart,
                // Cancellation safety: [`tokio::sync::mpsc::Receiver::recv`] is cancellation safe
                _ = self.slot_end.recv() => Next::SlotEnded,
            };

            // We need to store the current line, as the "request" variables lifetime is bound to it
            let mut _current_line = String::new();
            let response = match next {
                // User send some input
                Next::ClientInput(line) => {
                    let Some(line) = line else {
                        // The client closed the connection
                        return Ok(());
                    };

                    _current_line = match line {
                        Ok(line) => line,
                        Err(LinesCodecError::MaxLineLengthExceeded) => {
                            framed
                                .send(format!("ERROR The request line was too long. You can send at a maximum {MAX_INPUT_LINE_LENGTH} characters before you need to send a newline"))
                                .await
                                .context("Failed to send response to client")?;
                            return Ok(());
                        }
                        Err(err) => {
                            Err(err).context("Failed to read next line from framed LinesCodec")?
                        }
                    };
                    if _current_line.is_empty() {
                        continue;
                    }

                    let request =
                        Self::parse_request_report_errors(&_current_line, &mut framed).await?;
                    trace!(?request, "Got request");

                    match request {
                        None => None,
                        Some(request) => self
                            .determine_response(request)
                            .await
                            .context("Failed to process request")?,
                    }
                }
                // The slot to draw pixels started
                Next::SlotStart => {
                    self.currently_in_slot = true;
                    self.current_pixel_count = 0;

                    Some(Response::Start {
                        max_pixels_per_slot: self.max_pixels_per_slot,
                        max_slot_duration: self.max_slot_duration,
                    })
                }
                // The slot to draw pixels ended
                Next::SlotEnded => {
                    if self.currently_in_slot {
                        // The client did not send DONE in time, let's send a warning (might be an error in the future)
                        Some(Response::SlotNotClosedInTime {
                            max_slot_duration: self.max_slot_duration,
                        })
                    } else {
                        // Client did everything right, nothing to do
                        None
                    }
                }
            };

            // If there is no response to send we can process the next request
            let Some(response) = response else {
                continue;
            };
            trace!(?response, "Sending response");

            let close_connection = self
                .send_response(response, &mut framed)
                .await
                .context("Failed to send response to client")?;
            if close_connection {
                return Ok(());
            }
        }
    }

    #[inline(always)]
    async fn parse_request_report_errors<'line>(
        line: &'line str,
        framed: &mut Framed<&mut TcpStream, LinesCodec>,
    ) -> anyhow::Result<Option<Request<'line>>> {
        match parse_request(line).finish() {
            Ok(("", request)) => Ok(Some(request)),
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

    async fn determine_response(
        &mut self,
        request: Request<'_>,
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
                    self.current_username = Some(username.to_owned());
                    Response::LoginSucceeded
                } else {
                    self.current_username = None;
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
            Request::SetPixel { x, y, rgba } => {
                if self.current_username.is_none() {
                    return Ok(Some(Response::LoginNeeded));
                }
                if !self.currently_in_slot {
                    return Ok(Some(Response::NotYourSlot));
                }
                if self.current_pixel_count >= self.max_pixels_per_slot {
                    return Ok(Some(Response::QuotaExceeded {
                        max_pixels_per_slot: self.max_pixels_per_slot,
                    }));
                }

                self.current_pixel_count += 1;
                self.painted.push(PixelUpdate { x, y, rgba });

                None
            }
            Request::Done => {
                self.currently_in_slot = false;

                let num_pixels = self.painted.len();
                let ws_update = self.shared_state.framebuffer.write().await.set_multi(
                    self.current_username
                        .as_ref()
                        .context("The current username is not know. This should never happen!")?,
                    &self.painted,
                );

                self.painted.clear();

                self.shared_state
                    .ws_message_tx
                    .send(ws_update)
                    .await
                    .context("Failed to send update to websocket message channel")?;

                Some(Response::Done { num_pixels })
            }
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
            Response::Start {
                max_pixels_per_slot,
                max_slot_duration,
            } => {
                framed
                    .send(format!("START {} {}", max_pixels_per_slot, max_slot_duration.as_millis()))
                    .await
            }
            Response::Done { num_pixels } => framed.send(format!("DONE {num_pixels}")).await,
            Response::NotYourSlot => {
                close_connection = true;
                framed
                    .send("ERROR It was not your time slot, please wait until you get a START command!")
                    .await
            }
            Response::QuotaExceeded {
                max_pixels_per_slot,
            } => {
                close_connection = true;
                framed
                    .send(&format!("ERROR Quota exceeded. You are only allowed to set {max_pixels_per_slot} pixels per slot, please play fair!"))
                    .await
            }
            Response::SlotNotClosedInTime { max_slot_duration } => {
                close_connection = true;
                framed
                    .send(&format!("ERROR Slot not closed in time. After you finished drawing your pixels you need to send \"DONE\" to signalize you are done. Your slot lasts {max_slot_duration:?}, you need to send \"DONE\" in that period of time (keep the network delay in mind)"))
                    .await
            },
        }
        .context("Failed to send response to client")?;

        Ok(close_connection)
    }
}
