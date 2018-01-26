use std::string::FromUtf8Error;

use futures::{Future, IntoFuture, Stream, future};
use hyper::{Error as HyperError, Method, Request, Response, StatusCode};
use hyper::error::UriError;
use tokio_service::Service;

use ::models::repo::RepoPath;

const GITHUB_USER_CONTENT_BASE_URI: &'static str = "https://raw.githubusercontent.com";

#[derive(Debug)]
pub enum RetrieveFileAtPathError {
    Uri(UriError),
    Transport(HyperError),
    Status(StatusCode),
    Decode(FromUtf8Error)
}

pub fn retrieve_file_at_path<S>(service: S, repo_path: &RepoPath, file_path: &str) ->
    impl Future<Item=String, Error=RetrieveFileAtPathError>
    where S: Service<Request=Request, Response=Response, Error=HyperError>
{
    let uri_future = format!("{}/{}/{}/master/{}",
        GITHUB_USER_CONTENT_BASE_URI,
        repo_path.qual.as_ref(),
        repo_path.name.as_ref(),
        file_path
    ).parse().into_future().map_err(RetrieveFileAtPathError::Uri);

    uri_future.and_then(move |uri| {
        let request = Request::new(Method::Get, uri);

        service.call(request).map_err(RetrieveFileAtPathError::Transport).and_then(|response| {
            let status = response.status();
            if !status.is_success() {
                future::Either::A(future::err(RetrieveFileAtPathError::Status(status)))
            } else {
                let body_future = response.body().concat2().map_err(RetrieveFileAtPathError::Transport);
                let decode_future = body_future
                    .and_then(|body| String::from_utf8(body.to_vec()).map_err(RetrieveFileAtPathError::Decode));
                future::Either::B(decode_future)
            }
        })
    })
}
