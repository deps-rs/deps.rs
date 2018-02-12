use failure::Error;
use futures::{Future, IntoFuture, Stream, future};
use hyper::{Error as HyperError, Method, Request, Response, Uri};
use hyper::header::UserAgent;
use relative_path::RelativePathBuf;
use tokio_service::Service;
use serde_json;

use ::models::repo::{Repository, RepoPath};

const GITHUB_API_BASE_URI: &'static str = "https://api.github.com";
const GITHUB_USER_CONTENT_BASE_URI: &'static str = "https://raw.githubusercontent.com";

pub fn get_manifest_uri(repo_path: &RepoPath, path: &RelativePathBuf) -> Result<Uri, ::hyper::error::UriError> {
    let path_str: &str = path.as_ref();
    Ok(format!("{}/{}/{}/HEAD/{}",
        GITHUB_USER_CONTENT_BASE_URI,
        repo_path.qual.as_ref(),
        repo_path.name.as_ref(),
        path_str
    ).parse::<Uri>()?)
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

#[derive(Debug, Clone)]
pub struct GetPopularRepos<S>(pub S);

impl<S> Service for GetPopularRepos<S>
    where S: Service<Request=Request, Response=Response, Error=HyperError> + Clone + 'static,
          S::Future: 'static
{
    type Request = ();
    type Response = Vec<Repository>;
    type Error = Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, _req: ()) -> Self::Future {
        let service = self.0.clone();

        let uri_future = format!("{}/search/repositories?q=language:rust&sort=stars", GITHUB_API_BASE_URI)
            .parse().into_future().from_err();

        Box::new(uri_future.and_then(move |uri| {
            let mut request = Request::new(Method::Get, uri);
            request.headers_mut().set(UserAgent::new("deps.rs"));

            service.call(request).from_err().and_then(|response| {
                let status = response.status();
                if !status.is_success() {
                    future::Either::A(future::err(format_err!("Status code {} for popular repo search", status)))
                } else {
                    let body_future = response.body().concat2().from_err();
                    let decode_future = body_future
                        .and_then(|body| serde_json::from_slice(body.as_ref()).map_err(|err| err.into()));
                    future::Either::B(decode_future.and_then(|search_response: GithubSearchResponse| {
                        search_response.items.into_iter().map(|item| {
                            let path = RepoPath::from_parts("github", &item.owner.login, &item.name)?;
                            Ok(Repository { path, description: item.description })
                        }).collect::<Result<Vec<_>, _>>()
                    }))
                }
            })
        }))
    }
}
