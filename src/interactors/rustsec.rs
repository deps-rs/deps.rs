use std::{sync::Arc, task::Context, task::Poll};

use anyhow::Error;
use futures::FutureExt as _;
use hyper::service::Service;
use rustsec::database::Database;

use crate::BoxFuture;

#[derive(Debug, Clone)]
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

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let client = self.client.clone();
        Self::fetch(client).boxed()
    }
}
