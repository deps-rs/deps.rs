use std::{task::Context, task::Poll};

use anyhow::{anyhow, Error};
use futures::{
    future::{err, ok, ready, BoxFuture},
    TryFutureExt,
};
use hyper::{
    body, header::USER_AGENT, service::Service, Body, Error as HyperError, Request, Response,
};
use relative_path::RelativePathBuf;

use crate::models::repo::RepoPath;

pub mod crates;
pub mod github;
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
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx).map_err(|err| err.into())
    }

    fn call(&mut self, req: (RepoPath, RelativePathBuf)) -> Self::Future {
        let (repo_path, path) = req;

        let uri = repo_path.to_usercontent_file_uri(&path);
        let uri = match uri {
            Ok(uri) => uri,
            Err(error) => return Box::pin(err(error)),
        };

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
