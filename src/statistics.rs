use std::collections::HashMap;

use anyhow::Context;
use tokio::sync::mpsc;

use crate::proto::{
    web_socket_message::Payload, UserStatistics, UserStatisticsUpdate, WebSocketMessage,
};

pub struct Statistics {
    ws_message_tx: mpsc::Sender<WebSocketMessage>,

    stats: HashMap<String, UserStats>,
}

pub struct UserStats {}

impl Statistics {
    pub fn new(ws_message_tx: mpsc::Sender<WebSocketMessage>) -> Self {
        Self {
            ws_message_tx,
            stats: HashMap::new(),
        }
    }

    pub async fn register_user(&mut self, username: &str) -> anyhow::Result<()> {
        if !self.stats.contains_key(username) {
            self.stats.insert(username.to_string(), UserStats {});
        }
        self.send_update().await?;

        Ok(())
    }

    pub async fn unregister_user(&mut self, username: &str) -> anyhow::Result<()> {
        self.stats.remove(username);
        self.send_update().await?;

        Ok(())
    }

    pub async fn send_update(&self) -> anyhow::Result<()> {
        let statistics = self
            .stats
            .iter()
            .map(|(username, _stats)| UserStatistics {
                username: username.to_string(),
                pixels_per_s: 1000,
                average_response_time_ms: 42,
            })
            .collect();

        let ws_message = WebSocketMessage {
            payload: Some(Payload::UserStatisticsUpdate(UserStatisticsUpdate {
                statistics,
            })),
        };

        tracing::warn!(?ws_message, "Sending stats");

        self.ws_message_tx
            .send(ws_message)
            .await
            .context("Failed to send update to websocket message channel")?;

        Ok(())
    }
}
