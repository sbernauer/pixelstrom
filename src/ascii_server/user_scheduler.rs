use std::{collections::VecDeque, sync::Arc, time::Duration};

use anyhow::Context;
use tokio::{
    sync::{mpsc, RwLock},
    time::interval,
};
use tracing::trace;

use super::client_connection::SlotEvent;
use crate::{
    app_state::AppState,
    proto::{web_socket_message::Payload, CurrentlyPaintingClient, WebSocketMessage},
};

pub struct UserScheduler {
    shared_state: Arc<AppState>,
    users_queue: RwLock<VecDeque<ActiveUser>>,

    slot_duration: Duration,
}

struct ActiveUser {
    username: String,
    slot_tx: mpsc::Sender<SlotEvent>,
}

impl UserScheduler {
    pub fn new(shared_state: Arc<AppState>, slot_duration: Duration) -> Self {
        Self {
            shared_state,
            users_queue: Default::default(),
            slot_duration,
        }
    }

    /// Registers the given user.
    ///
    /// Returns
    /// 1. A receiver when the slot for the given user *starts*
    /// 2. A receiver when the slot for the given user *ends*
    pub async fn register_user(&self, username: &str, slot_tx: mpsc::Sender<SlotEvent>) {
        let active_user = ActiveUser {
            username: username.to_owned(),
            slot_tx,
        };
        self.users_queue.write().await.push_back(active_user);
    }

    /// Unregisters the given user.
    pub async fn unregister_user(&self, username: &str) {
        self.users_queue
            .write()
            .await
            .retain(|u| u.username != username);
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut interval = interval(self.slot_duration);

        loop {
            let mut users_queue = self.users_queue.write().await;

            // Stop previous user
            if let Some(prev) = users_queue.pop_front() {
                trace!(username = prev.username, "Closing slot for");

                if prev.slot_tx.send(SlotEvent::SlotEnd).await.is_ok() {
                    // Put user back in queue (all the way at the back)
                    users_queue.push_back(prev);
                }
            }

            if let Some(next) = users_queue.front() {
                trace!(username = next.username, "Next users turn");

                if next.slot_tx.send(SlotEvent::SlotStart).await.is_err() {
                    self.unregister_user(&next.username).await;
                }

                // let upcoming_users = users_queue
                //     .iter()
                //     .skip(1)
                //     .take(10)
                //     .map(|user| user.username.clone())
                //     .collect();
                let ws_message = WebSocketMessage {
                    payload: Some(Payload::CurrentlyPaintingClient(CurrentlyPaintingClient {
                        currently_painting: next.username.clone(),
                    })),
                };
                self.shared_state
                    .ws_message_tx
                    .send(ws_message)
                    .await
                    .context("Failed to send update to websocket message channel")?;
            } else {
                trace!("No user playing, no one for the next slot");
            }

            interval.tick().await;
        }
    }
}
