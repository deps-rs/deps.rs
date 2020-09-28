use failure::Error;
use futures::{Future, Stream};
use hyper::{Error as HyperError, Method, Request, Response};
use relative_path::RelativePathBuf;
use tokio_service::Service;

use crate::models::repo::{RepoPath, RepoSite};

pub mod bitbucket;
pub mod crates;
pub mod github;
pub mod gitlab;
pub mod rustsec;

#[derive(Debug, Clone)]
pub struct RetrieveFileAtPath<S>(pub S);

impl<S> Service for RetrieveFileAtPath<S>
where
    S: Service<Request = Request, Response = Response, Error = HyperError> + Clone + 'static,
    S::Future: 'static,
{
    type Request = (RepoPath, RelativePathBuf);
    type Response = String;
    type Error = Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let (repo_path, path) = req;
        let uri = match &repo_path.site {
            &RepoSite::Github => try_future_box!(github::get_manifest_uri(&repo_path, &path)),
            &RepoSite::Gitlab => try_future_box!(gitlab::get_manifest_uri(&repo_path, &path)),
            &RepoSite::Bitbucket => try_future_box!(bitbucket::get_manifest_uri(&repo_path, &path)),
        };

        let request = Request::new(Method::Get, uri.clone());

        Box::new(self.0.call(request).from_err().and_then(move |response| {
            let status = response.status();
            if !status.is_success() {
                try_future!(Err(format_err!("Status code {} for URI {}", status, uri)));
            }

            let body_future = response.body().concat2().from_err();

            body_future
                .and_then(|body| String::from_utf8(body.to_vec()).map_err(|err| err.into()))
                .into()
        }))
    }
}
