use std::fmt;

use actix_service::Service;
use anyhow::Error;
use futures_util::FutureExt as _;
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

#[derive(Clone)]
pub struct GetPopularRepos {
    client: reqwest::Client,
}

impl GetPopularRepos {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn query(client: reqwest::Client) -> anyhow::Result<Vec<Repository>> {
        let url = format!("{GITHUB_API_BASE_URI}/search/repositories?q=language:rust&sort=stars");

        let res = client.get(&url).send().await?.error_for_status()?;
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

impl fmt::Debug for GetPopularRepos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GetPopularRepos")
    }
}

impl Service<()> for GetPopularRepos {
    type Response = Vec<Repository>;
    type Error = Error;
    type Future = BoxFuture<Result<Self::Response, Self::Error>>;

    actix_service::always_ready!();

    fn call(&self, _req: ()) -> Self::Future {
        let client = self.client.clone();
        Self::query(client).boxed()
    }
}
