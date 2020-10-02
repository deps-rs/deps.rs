use std::task::{Context, Poll};

use anyhow::{anyhow, Error};

use futures::FutureExt as _;
use hyper::service::Service;
use serde::Deserialize;

use crate::{
    models::repo::{RepoPath, Repository},
    BoxFuture,
};

const GITHUB_API_BASE_URI: &str = "https://api.github.com";

#[derive(Deserialize)]
struct GithubSearchResponse {
    items: Vec<GithubRepo>,
}

#[derive(Deserialize)]
struct GithubRepo {
    name: String,
    owner: GithubOwner,
    description: String,
}

#[derive(Deserialize)]
struct GithubOwner {
    login: String,
}

#[derive(Debug, Clone)]
pub struct GetPopularRepos {
    client: reqwest::Client,
}

impl GetPopularRepos {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn query(client: reqwest::Client) -> anyhow::Result<Vec<Repository>> {
        let url = format!(
            "{}/search/repositories?q=language:rust&sort=stars",
            GITHUB_API_BASE_URI
        );
        let res = client.get(&url).send().await?;

        if !res.status().is_success() {
            return Err(anyhow!(
                "Status code {} for popular repo search",
                res.status()
            ));
        }

        let summary: GithubSearchResponse = res.json().await?;

        summary
            .items
            .into_iter()
            .map(|item| {
                let path = RepoPath::from_parts("github", &item.owner.login, &item.name)?;

                Ok(Repository {
                    path,
                    description: item.description,
                })
            })
            .collect::<Result<Vec<_>, Error>>()
    }
}

impl Service<()> for GetPopularRepos {
    type Response = Vec<Repository>;
    type Error = Error;
    type Future = BoxFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let client = self.client.clone();
        Self::query(client).boxed()
    }
}
