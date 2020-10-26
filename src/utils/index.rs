use std::time::Duration;

use anyhow::{Error, Result};
use crates_index::Index;
use slog::{error, info, Logger};
use tokio::task::spawn_blocking;
use tokio::time::{self, Interval};

pub struct ManagedIndex {
    index: Index,
    update_interval: Interval,
    logger: Logger,
}

impl ManagedIndex {
    pub fn new(update_interval: Duration, logger: Logger) -> Self {
        // the index path is configurable through the `CARGO_HOME` env variable
        let index = Index::new_cargo_default();
        let update_interval = time::interval(update_interval);
        Self {
            index,
            update_interval,
            logger,
        }
    }

    pub fn index(&self) -> Index {
        self.index.clone()
    }

    pub async fn initial_clone(&mut self) -> Result<()> {
        let index = self.index();
        let logger = self.logger.clone();

        spawn_blocking(move || {
            if !index.exists() {
                info!(logger, "Cloning crates.io-index");
                index.retrieve()?;
            }
            Ok::<_, Error>(())
        })
        .await??;
        Ok(())
    }

    pub async fn refresh_at_interval(&mut self) {
        loop {
            if let Err(e) = self.refresh().await {
                error!(
                    self.logger,
                    "failed refreshing the crates.io-index, the operation will be retried: {}", e
                );
            }
            self.update_interval.tick().await;
        }
    }

    async fn refresh(&self) -> Result<()> {
        let index = self.index();

        spawn_blocking(move || index.retrieve_or_update()).await??;
        Ok(())
    }
}
