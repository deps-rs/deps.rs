use std::{future::Future, task::Poll};
use std::{pin::Pin, task::Context};

use anyhow::{anyhow, Error};
use futures::{
    future::{err, ok, ready},
    TryFutureExt,
};
use hyper::{
    body, header::USER_AGENT, service::Service, Body, Error as HyperError, Request, Response,
};
use relative_path::RelativePathBuf;

use crate::models::repo::{RepoPath, RepoSite};

pub mod bitbucket;
pub mod crates;
pub mod github;
pub mod gitlab;
pub mod rustsec;

#[derive(Debug, Clone)]
pub struct RetrieveFileAtPath<S>(pub S);

impl<S> Service<(RepoPath, RelativePathBuf)> for RetrieveFileAtPath<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = HyperError> + Clone,
    S::Future: Send + 'static,
{
    type Response = String;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx).map_err(|err| err.into())
    }

    fn call(&mut self, req: (RepoPath, RelativePathBuf)) -> Self::Future {
        let (repo_path, path) = req;
        let uri = match &repo_path.site {
            &RepoSite::Github => github::get_manifest_uri(&repo_path, &path),
            &RepoSite::Gitlab => gitlab::get_manifest_uri(&repo_path, &path),
            &RepoSite::Bitbucket => bitbucket::get_manifest_uri(&repo_path, &path),
        };

        if let Err(error) = uri {
            return Box::pin(err(error));
        }

        let uri = uri.unwrap();
        let request = Request::get(uri.clone())
            .header(USER_AGENT, "deps.rs")
            .body(Body::empty())
            .unwrap();

        Box::pin(
            self.0
                .call(request)
                .err_into()
                .and_then(move |response| {
                    let status = response.status();

                    if status.is_success() {
                        ok(response)
                    } else {
                        err(anyhow!("Status code {} for URI {}", status, uri))
                    }
                })
                .and_then(|response| body::to_bytes(response.into_body()).err_into())
                .and_then(|bytes| ready(String::from_utf8(bytes.to_vec())).err_into()),
        )
    }
}
