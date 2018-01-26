use std::collections::BTreeMap;

use futures::{Future, future};
use hyper::{Error as HyperError, Request, Response, StatusCode};
use hyper::header::ContentType;
use semver::{Version, VersionReq};
use serde_json;
use slog::Logger;
use tokio_service::Service;

use ::models::repo::RepoPath;
use ::engine::Engine;

pub struct Api {
    pub engine: Engine
}

#[derive(Debug, Serialize)]
struct AnalyzeDependenciesResponseDetail {
    required: VersionReq,
    latest: Option<Version>,
    outdated: bool
}

#[derive(Debug, Serialize)]
struct AnalyzeDependenciesResponse {
    dependencies: BTreeMap<String, AnalyzeDependenciesResponseDetail>,
    #[serde(rename="dev-dependencies")]
    dev_dependencies: BTreeMap<String, AnalyzeDependenciesResponseDetail>,
    #[serde(rename="build-dependencies")]
    build_dependencies: BTreeMap<String, AnalyzeDependenciesResponseDetail>
}

impl Service for Api {
    type Request = Request;
    type Response = Response;
    type Error = HyperError;
    type Future = Box<Future<Item=Response, Error=HyperError>>;

    fn call(&self, req: Request) -> Self::Future {
        let repo_path = RepoPath::from_parts("github.com", "hyperium", "hyper").unwrap();

        let future = self.engine.analyze_dependencies(repo_path).then(|result| {
            match result {
                Err(err) => {
                    let mut response = Response::new();
                    response.set_status(StatusCode::InternalServerError);
                    response.set_body(format!("{:?}", err));
                    future::Either::A(future::ok(response))
                },
                Ok(dependencies) => {
                    let response_struct = AnalyzeDependenciesResponse {
                        dependencies: dependencies.main.into_iter()
                            .map(|(name, analyzed)| (name.into(), AnalyzeDependenciesResponseDetail {
                                outdated: analyzed.is_outdated(),
                                required: analyzed.required,
                                latest: analyzed.latest
                            })).collect(),
                        dev_dependencies: dependencies.dev.into_iter()
                            .map(|(name, analyzed)| (name.into(), AnalyzeDependenciesResponseDetail {
                                outdated: analyzed.is_outdated(),
                                required: analyzed.required,
                                latest: analyzed.latest
                            })).collect(),
                        build_dependencies: dependencies.build.into_iter()
                            .map(|(name, analyzed)| (name.into(), AnalyzeDependenciesResponseDetail {
                                outdated: analyzed.is_outdated(),
                                required: analyzed.required,
                                latest: analyzed.latest
                            })).collect()
                    };
                    let mut response = Response::new()
                        .with_header(ContentType::json())
                        .with_body(serde_json::to_string(&response_struct).unwrap());
                    future::Either::B(future::ok(response))
                }
            }
        });

        Box::new(future)
    }
}
