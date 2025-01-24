use std::{collections::VecDeque, time::Duration};

use tokio::{
    sync::{mpsc, RwLock},
    time::interval,
};
use tracing::trace;

use super::client_connection::SlotEvent;

pub struct UserScheduler {
    users_queue: RwLock<VecDeque<ActiveUser>>,

    slot_duration: Duration,
}

struct ActiveUser {
    username: String,
    slot_tx: mpsc::Sender<SlotEvent>,
}

impl UserScheduler {
    pub fn new(slot_duration: Duration) -> Self {
        Self {
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

        // let mut active_users = self.active_users.write().await;
        // let mut current_user_index = self.current_user_index.write().await;

        // if let Some(pos) = active_users.iter().position(|p| p.username == username) {
        //     active_users.remove(pos);

        //     // Adjust index if necessary
        //     if pos <= *current_user_index && *current_user_index > 0 {
        //         *current_user_index -= 1;
        //     }

        //     // Keep index in bounds
        //     *current_user_index %= active_users.len().max(1);
        // }
    }

    pub async fn run(&self) {
        let mut interval = interval(self.slot_duration);

        loop {
            // // Stop previous user
            // if let Some(ref slot_end_tx) = prev_slot_end_tx {
            //     if let Err(err) = slot_end_tx.send(()).await {
            //         error!(
            //             error = &err as &dyn std::error::Error,
            //             "Failed to send slot ended, unregistering user"
            //         );
            //         prev_slot_end_tx = None;
            //     }
            // }
            // self.move_to_next_user().await;

            // if let Some(current_user) = self
            //     .active_users
            //     .read()
            //     .await
            //     .get(*self.current_user_index.read().await)
            // {
            //     trace!(username = current_user.username, "Next slot started");

            //     if let Err(err) = current_user.slot_start_tx.send(()).await {
            //         error!(
            //             error = &err as &dyn std::error::Error,
            //             username = current_user.username,
            //             "Failed to send slot started"
            //         );
            //         prev_slot_end_tx = None;
            //     } else {
            //         prev_slot_end_tx = Some(current_user.slot_end_tx.clone());
            //         prev_slot_end_tx
            //     }
            // } else {
            //     trace!("No current user to use this slow");
            // };

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
            } else {
                trace!("No user playing, no one for the next slot");
            }

            interval.tick().await;
        }
    }
}
