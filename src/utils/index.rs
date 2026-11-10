use std::{fs, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use crates_index::{Crate, GitIndex};
use parking_lot::Mutex;
use tokio::{
    task::spawn_blocking,
    time::{self, MissedTickBehavior},
};

use crate::models::crates::CrateName;

#[derive(Clone)]
pub struct ManagedIndex {
    index: Arc<Mutex<Option<GitIndex>>>,
}

impl ManagedIndex {
    pub fn new() -> Self {
        // the index path is configurable through the `CARGO_HOME` env variable
        let index = Arc::new(Mutex::new(Some(GitIndex::new_cargo_default().unwrap())));

        Self { index }
    }

    pub fn crate_(&self, crate_name: CrateName) -> Option<Crate> {
        self.index
            .lock()
            .as_ref()
            .expect("ManagedIndex is poisoned")
            .crate_(crate_name.as_ref())
    }

    pub async fn refresh_at_interval(&self, update_interval: Duration) {
        let mut update_interval = time::interval(update_interval);
        update_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            if let Err(err) = self.refresh().await {
                tracing::error!(
                    "failed refreshing the crates.io-index, the operation will be retried: {err:#}"
                );
            }
            update_interval.tick().await;
        }
    }

    async fn refresh(&self) -> Result<()> {
        let this_index = Arc::clone(&self.index);

        spawn_blocking(move || {
            let mut index = this_index.lock();
            let git_index = index.as_mut().context("ManagedIndex is poisoned")?;

            match git_index.update() {
                Ok(()) => Ok(()),
                Err(err) => match current_entries(&err) {
                    Some(..4096) => {
                        tracing::info!(
                            "Reopening crates.io-index to make gix expand the internal slotmap"
                        );
                        *git_index = GitIndex::with_path(git_index.path(), git_index.url())
                            .context("could not reopen git index")?;
                        git_index
                            .update()
                            .context("failed to update crates.io-index after `git gc`")
                    }
                    Some(4096..) => {
                        tracing::info!(
                            "Cloning a new crates.io-index and replacing it with the current one"
                        );
                        let path = git_index.path().to_owned();
                        let url = git_index.url().to_owned();

                        // Avoid keeping the index locked for too long
                        drop(index);

                        // Clone the new index
                        let mut tmp_path = path.clone();
                        tmp_path.as_mut_os_string().push(".new");
                        if tmp_path.try_exists()? {
                            fs::remove_dir_all(&tmp_path)?;
                        }
                        let new_index = GitIndex::with_path(&tmp_path, &url)
                            .context("could not clone new git index")?;

                        // Swap the old index with the new one
                        drop(new_index);

                        let mut index = this_index.lock();
                        *index = None;
                        // NOTE: if any of the following operations fail,
                        // the index is poisoned
                        fs::remove_dir_all(&path)?;
                        fs::rename(tmp_path, &path)?;

                        *index = Some(
                            GitIndex::with_path(path, url).context("could not reopen git index")?,
                        );
                        Ok(())
                    }
                    None => {
                        Err(anyhow::Error::from(err).context("failed to update crates.io-index"))
                    }
                },
            }
        })
        .await
        .expect("blocking index update task should never panic")?;

        Ok(())
    }
}

fn current_entries(err: &crates_index::Error) -> Option<usize> {
    let crates_index::Error::Git(crates_index::error::GixError::Gix(err)) = err else {
        return None;
    };

    // gix reports running out of object database index slots
    // as a message carrying the `current` and `needed` slot counts
    err.metadata().find_map(|metadata| {
        let gix::error::MetadataValue::U64(current) = metadata.get("current")? else {
            return None;
        };
        let gix::error::MetadataValue::U64(needed) = metadata.get("needed")? else {
            return None;
        };

        usize::try_from(current.checked_add(*needed)?).ok()
    })
}

#[cfg(test)]
mod tests {
    use gix::error::{ErrorExt as _, Message};

    use super::current_entries;

    fn index_error(err: gix::Error) -> crates_index::Error {
        crates_index::Error::Git(crates_index::error::GixError::Gix(err))
    }

    #[test]
    fn current_entries_of_insufficient_slots() {
        let insufficient_slots = Message::new("The object database has too few index slots")
            .with("current", 1024_usize)
            .with("needed", 3_usize)
            .raise_typed();
        let err = Message::new("Failed to update references")
            .raise_typed()
            .chain(insufficient_slots);

        assert_eq!(current_entries(&index_error(err.into())), Some(1027));
    }

    #[test]
    fn current_entries_of_other_errors() {
        let err = Message::new("Could not connect to remote").raise();

        assert_eq!(current_entries(&index_error(err)), None);
    }
}
