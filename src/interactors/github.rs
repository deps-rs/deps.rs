use std::string::FromUtf8Error;

use futures::{Future, IntoFuture, Stream, future};
use hyper::{Error as HyperError, Method, Request, Response, StatusCode};
use hyper::error::UriError;
use hyper::header::UserAgent;
use tokio_service::Service;
use serde_json;

use ::models::repo::{Repository, RepoPath, RepoValidationError};

const GITHUB_API_BASE_URI: &'static str = "https://api.github.com";
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

#[derive(Debug)]
pub enum GetPopularReposError {
    Uri(UriError),
    Transport(HyperError),
    Status(StatusCode),
    Decode(serde_json::Error),
    Validate(RepoValidationError)
}

#[derive(Deserialize)]
struct GithubSearchResponse {
    items: Vec<GithubRepo>
}

#[derive(Deserialize)]
struct GithubRepo {
    name: String,
    owner: GithubOwner,
    description: String
}

#[derive(Deserialize)]
struct GithubOwner {
    login: String
}

pub fn get_popular_repos<S>(service: S) ->
    impl Future<Item=Vec<Repository>, Error=GetPopularReposError>
    where S: Service<Request=Request, Response=Response, Error=HyperError>
{
    let uri_future = format!("{}/search/repositories?q=language:rust&sort=stars", GITHUB_API_BASE_URI)
        .parse().into_future().map_err(GetPopularReposError::Uri);

    uri_future.and_then(move |uri| {
        let mut request = Request::new(Method::Get, uri);
        request.headers_mut().set(UserAgent::new("deps.rs"));

        service.call(request).map_err(GetPopularReposError::Transport).and_then(|response| {
            let status = response.status();
            if !status.is_success() {
                future::Either::A(future::err(GetPopularReposError::Status(status)))
            } else {
                let body_future = response.body().concat2().map_err(GetPopularReposError::Transport);
                let decode_future = body_future
                    .and_then(|body| serde_json::from_slice(body.as_ref()).map_err(GetPopularReposError::Decode));
                future::Either::B(decode_future.and_then(|search_response: GithubSearchResponse| {
                    search_response.items.into_iter().map(|item| {
                        let path = RepoPath::from_parts("github", &item.owner.login, &item.name)
                            .map_err(GetPopularReposError::Validate)?;
                        Ok(Repository { path, description: item.description })
                    }).collect::<Result<Vec<_>, _>>()
                }))
            }
        })
    })
}
