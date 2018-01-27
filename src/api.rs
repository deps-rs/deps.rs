use std::collections::BTreeMap;
use std::sync::Arc;

use futures::{Future, IntoFuture, future};
use hyper::{Error as HyperError, Method, Request, Response, StatusCode};
use hyper::header::ContentType;
use route_recognizer::{Params, Router};
use semver::{Version, VersionReq};
use serde_json;
use slog::Logger;
use tokio_service::Service;

use ::models::repo::RepoPath;
use ::engine::Engine;

enum Route {
    AnalyzeDependencies
}

#[derive(Clone)]
pub struct Api {
    engine: Engine,
    router: Arc<Router<Route>>
}

impl Api {
    pub fn new(engine: Engine) -> Api {
        let mut router = Router::new();
        router.add("/api/v1/analyze/:site/:qual/:name", Route::AnalyzeDependencies);

        Api { engine, router: Arc::new(router) }
    }
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
        if let Ok(route_match) = self.router.recognize(req.uri().path()) {
            match route_match.handler {
                &Route::AnalyzeDependencies => {
                    if *req.method() == Method::Get {
                        return Box::new(self.analyze_dependencies(req, route_match.params));
                    }
                }
            }
        }

        let mut response = Response::new();
        response.set_status(StatusCode::NotFound);
        Box::new(future::ok(response))
    }
}

impl Api {
    fn analyze_dependencies<'r>(&self, _req: Request, params: Params) -> impl Future<Item=Response, Error=HyperError> {
        let engine = self.engine.clone();

        let site = params.find("site").expect("route param 'site' not found");
        let qual = params.find("qual").expect("route param 'qual' not found");
        let name = params.find("name").expect("route param 'name' not found");

        RepoPath::from_parts(site, qual, name).into_future().then(move |repo_path_result| {
            match repo_path_result {
                Err(err) => {
                    let mut response = Response::new();
                    response.set_status(StatusCode::BadRequest);
                    response.set_body(format!("{:?}", err));
                    future::Either::A(future::ok(response))
                },
                Ok(repo_path) => {
                    future::Either::B(engine.analyze_dependencies(repo_path).then(|analyze_result| {
                        match analyze_result {
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
                    }))
                }
            }
        })
    }
}