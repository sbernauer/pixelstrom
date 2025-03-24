use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Context;
use simple_moving_average::{SumTreeSMA, SMA};
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
    pub average_pixels_per_round: SumTreeSMA<f32, f32, 10>,
    pub average_response_time: SumTreeSMA<Duration, u32, 10>,
}

impl UserStats {
    pub fn new() -> Self {
        Self {
            average_pixels_per_round: SumTreeSMA::new(),
            average_response_time: SumTreeSMA::from_zero(Duration::from_nanos(0)),
        }
    }
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
        let mut interval = interval(Duration::from_millis(500));
        loop {
            interval.tick().await;

            self.send_update()
                .await
                .context("failed to send statistics update")?;
        }
    }

    pub async fn record(&self, username: impl Into<String>, pixels: u32, response_time: Duration) {
        let mut stats = self.stats.write().await;
        let user_stats = stats.entry(username.into()).or_insert_with(UserStats::new);

        user_stats
            .average_pixels_per_round
            .add_sample(pixels as f32);
        user_stats.average_response_time.add_sample(response_time);
    }

    pub async fn send_update(&self) -> anyhow::Result<()> {
        let all_users_as_ordered_list = self.user_scheduler.all_users_as_ordered_list().await;
        let current_stats = self.stats.read().await;
        let statistics = all_users_as_ordered_list
            .iter()
            .map(|username| match current_stats.get(username) {
                Some(stats) => UserStatistics {
                    username: username.to_string(),
                    average_pixels_per_round: stats.average_pixels_per_round.get_average(),
                    average_response_time_milliseconds: stats
                        .average_response_time
                        .get_average()
                        // TODO: Switch to [`Duration::as_millis_f32`] once stable
                        .as_secs_f32()
                        * 1000.0,
                },

                // We don't have any stats, so let's ship empty ones
                // We need to send *something*, so that the user is contained in the users list
                None => UserStatistics {
                    username: username.to_string(),
                    average_pixels_per_round: 0.0,
                    average_response_time_milliseconds: 0.0,
                },
            })
            .collect::<Vec<_>>();

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
