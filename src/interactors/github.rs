use std::{future::Future, pin::Pin, task::Context, task::Poll};

use anyhow::{anyhow, Error};
use futures::{
    future::{err, ok, ready},
    TryFutureExt,
};
use hyper::{
    body, header::USER_AGENT, service::Service, Body, Error as HyperError, Request, Response, Uri,
};
use serde::Deserialize;

use crate::models::repo::{RepoPath, Repository};

const GITHUB_API_BASE_URI: &'static str = "https://api.github.com";

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
pub struct GetPopularRepos<S>(pub S);

impl<S> Service<()> for GetPopularRepos<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = HyperError> + Clone,
    S::Future: Send + 'static,
{
    type Response = Vec<Repository>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx).map_err(|err| err.into())
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let uri = format!(
            "{}/search/repositories?q=language:rust&sort=stars",
            GITHUB_API_BASE_URI
        )
        .parse::<Uri>()
        .expect("TODO: handle error properly");

        let request = Request::get(uri)
            .header(USER_AGENT, "deps.rs")
            .body(Body::empty())
            .unwrap();

        Box::pin(
            self.0
                .call(request)
                .err_into()
                .and_then(|response| {
                    let status = response.status();
                    if !status.is_success() {
                        return err(anyhow!("Status code {} for popular repo search", status));
                    }

                    ok(response)
                })
                .and_then(|response| body::to_bytes(response.into_body()).err_into())
                .and_then(|bytes| ready(serde_json::from_slice(bytes.as_ref())).err_into())
                .and_then(|search_response: GithubSearchResponse| {
                    ready(
                        search_response
                            .items
                            .into_iter()
                            .map(|item| {
                                let path =
                                    RepoPath::from_parts("github", &item.owner.login, &item.name)?;

                                Ok(Repository {
                                    path,
                                    description: item.description,
                                })
                            })
                            .collect::<Result<Vec<_>, Error>>(),
                    )
                }),
        )
    }
}
