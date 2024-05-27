use std::fmt;

use actix_web::dev::Service;
use anyhow::{anyhow, Error};
use futures_util::{future::LocalBoxFuture, FutureExt as _};
use relative_path::RelativePathBuf;

use crate::models::repo::RepoPath;

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
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::always_ready!();

    fn call(&self, (repo_path, path): (RepoPath, RelativePathBuf)) -> Self::Future {
        let client = self.client.clone();
        Self::query(client, repo_path, path).boxed()
    }
}

impl fmt::Debug for RetrieveFileAtPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RetrieveFileAtPath")
    }
}
