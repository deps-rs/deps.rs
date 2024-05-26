use std::{fmt, sync::Arc};

use actix_service::Service;
use anyhow::Error;
use futures_util::FutureExt as _;
use rustsec::database::Database;

use crate::BoxFuture;

#[derive(Clone)]
pub struct FetchAdvisoryDatabase {
    client: reqwest::Client,
}

impl FetchAdvisoryDatabase {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn fetch(_client: reqwest::Client) -> anyhow::Result<Arc<Database>> {
        // TODO: make fetch async
        Ok(rustsec::Database::fetch().map(Arc::new)?)
    }
}

impl Service<()> for FetchAdvisoryDatabase {
    type Response = Arc<Database>;
    type Error = Error;
    type Future = BoxFuture<Result<Self::Response, Self::Error>>;

    actix_service::always_ready!();

    fn call(&self, _req: ()) -> Self::Future {
        let client = self.client.clone();
        Self::fetch(client).boxed()
    }
}

impl fmt::Debug for FetchAdvisoryDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FetchAdvisoryDatabase")
    }
}
