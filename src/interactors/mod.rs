use std::{
    fmt,
    task::{Context, Poll},
};

use anyhow::{anyhow, Error};
use futures_util::FutureExt as _;
use hyper::service::Service;
use relative_path::RelativePathBuf;

use crate::{models::repo::RepoPath, BoxFuture};

pub mod crates;
pub mod github;
pub mod rustsec;

#[derive(Clone)]
pub struct RetrieveFileAtPath {
    client: reqwest::Client,
}

impl RetrieveFileAtPath {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn query(
        client: reqwest::Client,
        repo_path: RepoPath,
        path: RelativePathBuf,
    ) -> anyhow::Result<String> {
        let url = repo_path.to_usercontent_file_url(&path);
        let res = client.get(&url).send().await?;

        if !res.status().is_success() {
            return Err(anyhow!("Status code {} for URI {}", res.status(), url));
        }

        Ok(res.text().await?)
    }
}

impl Service<(RepoPath, RelativePathBuf)> for RetrieveFileAtPath {
    type Response = String;
    type Error = Error;
    type Future = BoxFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, (repo_path, path): (RepoPath, RelativePathBuf)) -> Self::Future {
        let client = self.client.clone();
        Self::query(client, repo_path, path).boxed()
    }
}

impl fmt::Debug for RetrieveFileAtPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RetrieveFileAtPath")
    }
}
