use std::{collections::VecDeque, time::Duration};

use anyhow::Context;
use tokio::{
    sync::{mpsc, RwLock},
    time::interval,
};
use tracing::trace;

use super::client_connection::SlotEvent;
use crate::proto::{web_socket_message::Payload, CurrentlyPaintingUser, WebSocketMessage};

pub struct UserScheduler {
    ws_message_tx: mpsc::Sender<WebSocketMessage>,

    /// All the active users in the order of joining.
    active_users: RwLock<VecDeque<String>>,

    /// The queue of active users, the first one in the list will always get the next turn.
    users_queue: RwLock<VecDeque<ActiveUser>>,
    slot_duration: Duration,
}

struct ActiveUser {
    username: String,
    slot_tx: mpsc::Sender<SlotEvent>,
}

impl UserScheduler {
    pub fn new(ws_message_tx: mpsc::Sender<WebSocketMessage>, slot_duration: Duration) -> Self {
        Self {
            ws_message_tx,
            active_users: Default::default(),
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
            username: username.to_string(),
            slot_tx,
        };

        // As we store them in the order of joining this new user comes last
        self.active_users
            .write()
            .await
            .push_back(username.to_string());

        // Users start at the very end of the queue
        self.users_queue.write().await.push_back(active_user);
    }

    /// Unregisters the given user.
    pub async fn unregister_user(&self, username: &str) -> anyhow::Result<()> {
        self.active_users.write().await.retain(|u| u != username);
        self.users_queue
            .write()
            .await
            .retain(|u| u.username != username);

        Ok(())
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
                    self.unregister_user(&next.username).await?;
                }

                // let upcoming_users = users_queue
                //     .iter()
                //     .skip(1)
                //     .take(10)
                //     .map(|user| user.username.clone())
                //     .collect();
                let ws_message = WebSocketMessage {
                    payload: Some(Payload::CurrentlyPaintingUser(CurrentlyPaintingUser {
                        currently_painting: next.username.clone(),
                    })),
                };
                self.ws_message_tx
                    .send(ws_message)
                    .await
                    .context("Failed to send update to websocket message channel")?;
            } else {
                trace!("No user playing, no one for the next slot");
            }

            interval.tick().await;
        }
    }

    /// Return the users in the order of playing. The list alway starts with the oldest user.
    pub async fn all_users_as_ordered_list(&self) -> Vec<String> {
        let mut ordered_users = self
            .users_queue
            .read()
            .await
            .iter()
            .map(|user| user.username.to_string())
            .collect::<Vec<_>>();
        let Some(oldest_user) = self.active_users.read().await.front().cloned() else {
            // There are currently no active users.
            return vec![];
        };

        let oldest_user_pos = ordered_users
            .iter()
            .position(|username| username == &oldest_user)
            // Not sure what the oldest ust is, so let's keep the list unchanged
            .unwrap_or_default();

        // Rotate the list in such a way that the oldest player is always the first one
        ordered_users.rotate_left(oldest_user_pos);

        ordered_users
    }
}
