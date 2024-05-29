use std::{sync::Arc, time::Duration};

use anyhow::Result;
use crates_index::{Crate, GitIndex};
use parking_lot::Mutex;
use tokio::{
    task::spawn_blocking,
    time::{self, MissedTickBehavior},
};

use crate::models::crates::CrateName;

#[derive(Clone)]
pub struct ManagedIndex {
    index: Arc<Mutex<GitIndex>>,
}

impl ManagedIndex {
    pub fn new() -> Self {
        // the index path is configurable through the `CARGO_HOME` env variable
        let index = Arc::new(Mutex::new(GitIndex::new_cargo_default().unwrap()));

        Self { index }
    }

    pub fn crate_(&self, crate_name: CrateName) -> Option<Crate> {
        self.index.lock().crate_(crate_name.as_ref())
    }

    pub async fn refresh_at_interval(&self, update_interval: Duration) {
        let mut update_interval = time::interval(update_interval);
        update_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            if let Err(err) = self.refresh().await {
                tracing::error!(
                    "failed refreshing the crates.io-index, the operation will be retried: {}",
                    error_reporter::Report::new(err),
                );
            }
            update_interval.tick().await;
        }
    }

    async fn refresh(&self) -> Result<(), crates_index::Error> {
        let index = Arc::clone(&self.index);

        spawn_blocking(move || index.lock().update())
            .await
            .expect("blocking index update task should never panic")?;

        Ok(())
    }
}
