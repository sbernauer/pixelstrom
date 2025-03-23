use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Context;
use tokio::{
    sync::{mpsc, RwLock},
    time::interval,
};

use crate::{
    ascii_server::user_scheduler::UserScheduler,
    proto::{web_socket_message::Payload, UserStatistics, UserStatisticsUpdate, WebSocketMessage},
};

pub struct Statistics {
    ws_message_tx: mpsc::Sender<WebSocketMessage>,
    user_scheduler: Arc<UserScheduler>,

    stats: RwLock<HashMap<String, UserStats>>,
}

pub struct UserStats {
    pub pixels_per_second: f32,
    pub average_response_time_milliseconds: f32,
}

impl Statistics {
    pub fn new(
        ws_message_tx: mpsc::Sender<WebSocketMessage>,
        user_scheduler: Arc<UserScheduler>,
    ) -> Self {
        Self {
            ws_message_tx,
            user_scheduler,
            stats: RwLock::new(HashMap::new()),
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut interval = interval(Duration::from_millis(100)); // FIXME: Reduce
        loop {
            interval.tick().await;

            self.send_update()
                .await
                .context("failed to send statistics update")?;
        }
    }

    pub async fn send_update(&self) -> anyhow::Result<()> {
        let all_users_as_ordered_list = self.user_scheduler.all_users_as_ordered_list().await;
        let current_stats = self.stats.read().await;
        let statistics = all_users_as_ordered_list
            .iter()
            .map(|username| match current_stats.get(username) {
                Some(stats) => UserStatistics {
                    username: username.to_string(),
                    pixels_per_second: stats.pixels_per_second,
                    average_response_time_milliseconds: stats.average_response_time_milliseconds,
                },
                None => UserStatistics {
                    username: username.to_string(),
                    pixels_per_second: 0.0,
                    average_response_time_milliseconds: 0.0,
                },
            })
            .collect();

        let ws_message = WebSocketMessage {
            payload: Some(Payload::UserStatisticsUpdate(UserStatisticsUpdate {
                statistics,
            })),
        };

        self.ws_message_tx
            .send(ws_message)
            .await
            .context("Failed to send update to websocket message channel")?;

        Ok(())
    }
}
