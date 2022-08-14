use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use crate::models::crates::CrateName;
use anyhow::Result;
use crates_index::Crate;
use crates_index::Index;
use slog::{error, Logger};
use tokio::task::spawn_blocking;
use tokio::time::{self, MissedTickBehavior};

#[derive(Clone)]
pub struct ManagedIndex {
    index: Arc<Mutex<Index>>,
    logger: Logger,
}

impl ManagedIndex {
    pub fn new(logger: Logger) -> Self {
        // the index path is configurable through the `CARGO_HOME` env variable
        let index = Arc::new(Mutex::new(Index::new_cargo_default().unwrap()));
        Self { index, logger }
    }

    pub fn crate_(&self, crate_name: CrateName) -> Option<Crate> {
        let index = self.index.lock().unwrap();

        index.crate_(crate_name.as_ref())
    }

    pub async fn refresh_at_interval(&self, update_interval: Duration) {
        let mut update_interval = time::interval(update_interval);
        update_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            if let Err(e) = self.refresh().await {
                error!(
                    self.logger,
                    "failed refreshing the crates.io-index, the operation will be retried: {}", e
                );
            }
            update_interval.tick().await;
        }
    }

    async fn refresh(&self) -> Result<()> {
        let index = Arc::clone(&self.index);

        spawn_blocking(move || {
            let mut index = index.lock().unwrap();

            index.update()
        })
        .await??;
        Ok(())
    }
}
