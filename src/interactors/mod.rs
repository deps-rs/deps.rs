use failure::Error;
use futures::{Future, IntoFuture, Stream, future};
use hyper::{Error as HyperError, Method, Request, Response};
use relative_path::RelativePathBuf;
use tokio_service::Service;

use ::models::repo::{RepoSite, RepoPath};

pub mod bitbucket;
pub mod crates;
pub mod github;
pub mod gitlab;
pub mod rustsec;

#[derive(Debug, Clone)]
pub struct RetrieveFileAtPath<S>(pub S);

impl<S> Service for RetrieveFileAtPath<S>
    where S: Service<Request=Request, Response=Response, Error=HyperError> + Clone + 'static,
          S::Future: 'static
{
    type Request = (RepoPath, RelativePathBuf);
    type Response = String;
    type Error = Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let service = self.0.clone();

        let (repo_path, path) = req;
        let uri = match &repo_path.site {
            &RepoSite::Github => {
                github::get_manifest_uri(&repo_path, &path)
            },
            &RepoSite::Gitlab => {
                gitlab::get_manifest_uri(&repo_path, &path)
            },
            &RepoSite::Bitbucket => {
                bitbucket::get_manifest_uri(&repo_path, &path)
            }
        };
        let uri_future = uri.into_future().from_err();

        Box::new(uri_future.and_then(move |uri| {
            let request = Request::new(Method::Get, uri.clone());

            service.call(request).from_err().and_then(move |response| {
                let status = response.status();
                if !status.is_success() {
                    future::Either::A(future::err(format_err!("Status code {} for URI {}", status, uri)))
                } else {
                    let body_future = response.body().concat2().from_err();
                    let decode_future = body_future
                        .and_then(|body| String::from_utf8(body.to_vec()).map_err(|err| err.into()));
                    future::Either::B(decode_future)
                }
            })
        }))
    }
}


