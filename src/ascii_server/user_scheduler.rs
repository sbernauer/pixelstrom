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
    pub async fn register_user(
        &self,
        username: &str,
        slot_tx: mpsc::Sender<SlotEvent>,
    ) -> anyhow::Result<()> {
        let active_user = ActiveUser {
            username: username.to_string(),
            slot_tx,
        };

        let mut users_queue = self.users_queue.write().await;
        let mut active_users = self.active_users.write().await;

        // Users start at the very end of the queue
        users_queue.push_back(active_user);

        // It's a bit more complex where in the queue the user got inserted.
        let current_user = users_queue
            .front()
            .expect("We just pushed one element, the users queue can not be empty");
        let current_user_pos = active_users
            .iter()
            .position(|username| username == &current_user.username)
            // If not found we are probably the only user. Let's stuff the user at the end
            .unwrap_or_else(|| active_users.len().saturating_sub(1));

        active_users.insert(current_user_pos, username.to_string());

        Ok(())
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

    pub async fn all_users_as_ordered_list(&self) -> Vec<String> {
        self.active_users.read().await.iter().cloned().collect()
    }
}
