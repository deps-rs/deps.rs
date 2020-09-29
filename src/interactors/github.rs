use anyhow::{anyhow, ensure, Error};
use futures::{Future, Stream};
use hyper::header::USER_AGENT;
use hyper::{Body, Error as HyperError, Method, Request, Response, Uri};
use relative_path::RelativePathBuf;
use serde::Deserialize;
use tokio_service::Service;

use crate::models::repo::{RepoPath, Repository};

const GITHUB_API_BASE_URI: &'static str = "https://api.github.com";
const GITHUB_USER_CONTENT_BASE_URI: &'static str = "https://raw.githubusercontent.com";

pub fn get_manifest_uri(repo_path: &RepoPath, path: &RelativePathBuf) -> Result<Uri, Error> {
    let path_str: &str = path.as_ref();
    Ok(format!(
        "{}/{}/{}/HEAD/{}",
        GITHUB_USER_CONTENT_BASE_URI,
        repo_path.qual.as_ref(),
        repo_path.name.as_ref(),
        path_str
    )
    .parse::<Uri>()?)
}

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

impl<S> Service for GetPopularRepos<S>
where
    S: Service<Request = Request<Body>, Response = Response<Body>, Error = HyperError>
        + Clone
        + 'static,
    S::Future: Send + 'static,
{
    type Request = ();
    type Response = Vec<Repository>;
    type Error = Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error> + Send>;

    fn call(&self, _req: ()) -> Self::Future {
        let uri = try_future_box!(format!(
            "{}/search/repositories?q=language:rust&sort=stars",
            GITHUB_API_BASE_URI
        )
        .parse::<Uri>());

        let mut request = Request::get(uri);
        request.header(USER_AGENT, "deps.rs");
        let request = request.body(Body::empty()).unwrap();

        Box::new(self.0.call(request).from_err().and_then(|response| {
            let status = response.status();
            if !status.is_success() {
                try_future!(Err(anyhow!(
                    "Status code {} for popular repo search",
                    status
                )));
            }

            let body_future = response.into_body().concat2().from_err();
            let decode_future = body_future
                .and_then(|body| serde_json::from_slice(body.as_ref()).map_err(|err| err.into()));
            decode_future
                .and_then(|search_response: GithubSearchResponse| {
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
                        .collect::<Result<Vec<_>, _>>()
                })
                .into()
        }))
    }
}
