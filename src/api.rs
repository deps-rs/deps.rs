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

use ::assets;
use ::engine::{Engine, AnalyzeDependenciesOutcome};
use ::models::repo::RepoPath;

#[derive(Clone, Copy)]
enum StatusFormat {
    Json,
    Svg
}

enum Route {
    Status(StatusFormat)
}

#[derive(Clone)]
pub struct Api {
    engine: Engine,
    router: Arc<Router<Route>>
}

impl Api {
    pub fn new(engine: Engine) -> Api {
        let mut router = Router::new();
        router.add("/repo/:site/:qual/:name/status.json", Route::Status(StatusFormat::Json));
        router.add("/repo/:site/:qual/:name/status.svg", Route::Status(StatusFormat::Svg));

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
struct AnalyzeDependenciesResponseSingle {
    dependencies: BTreeMap<String, AnalyzeDependenciesResponseDetail>,
    #[serde(rename="dev-dependencies")]
    dev_dependencies: BTreeMap<String, AnalyzeDependenciesResponseDetail>,
    #[serde(rename="build-dependencies")]
    build_dependencies: BTreeMap<String, AnalyzeDependenciesResponseDetail>
}

#[derive(Debug, Serialize)]
struct AnalyzeDependenciesResponse {
    crates: BTreeMap<String, AnalyzeDependenciesResponseSingle>
}

impl Service for Api {
    type Request = Request;
    type Response = Response;
    type Error = HyperError;
    type Future = Box<Future<Item=Response, Error=HyperError>>;

    fn call(&self, req: Request) -> Self::Future {
        if let Ok(route_match) = self.router.recognize(req.uri().path()) {
            match route_match.handler {
                &Route::Status(format) => {
                    if *req.method() == Method::Get {
                        return Box::new(self.status(req, route_match.params, format));
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
    fn status<'r>(&self, _req: Request, params: Params, format: StatusFormat) ->
        impl Future<Item=Response, Error=HyperError>
    {
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
                    future::Either::B(engine.analyze_dependencies(repo_path).then(move |analyze_result| {
                        match analyze_result {
                            Err(err) => {
                                let mut response = Response::new();
                                response.set_status(StatusCode::InternalServerError);
                                response.set_body(format!("{:?}", err));
                                future::Either::A(future::ok(response))
                            },
                            Ok(analysis_outcome) => {
                                let response = Api::status_format_analysis(analysis_outcome, format);
                                future::Either::B(future::ok(response))
                            }
                        }
                    }))
                }
            }
        })
    }

    fn status_format_analysis(analysis_outcome: AnalyzeDependenciesOutcome, format: StatusFormat) -> Response {
        match format {
            StatusFormat::Json => {
               let single = AnalyzeDependenciesResponseSingle {
                    dependencies: analysis_outcome.deps.main.into_iter()
                        .map(|(name, analyzed)| (name.into(), AnalyzeDependenciesResponseDetail {
                            outdated: analyzed.is_outdated(),
                            required: analyzed.required,
                            latest: analyzed.latest
                        })).collect(),
                    dev_dependencies: analysis_outcome.deps.dev.into_iter()
                        .map(|(name, analyzed)| (name.into(), AnalyzeDependenciesResponseDetail {
                            outdated: analyzed.is_outdated(),
                            required: analyzed.required,
                            latest: analyzed.latest
                        })).collect(),
                    build_dependencies: analysis_outcome.deps.build.into_iter()
                        .map(|(name, analyzed)| (name.into(), AnalyzeDependenciesResponseDetail {
                            outdated: analyzed.is_outdated(),
                            required: analyzed.required,
                            latest: analyzed.latest
                        })).collect()
                };
                let multi = AnalyzeDependenciesResponse {
                    crates: vec![(analysis_outcome.name.into(), single)].into_iter().collect()
                };
                Response::new()
                    .with_header(ContentType::json())
                    .with_body(serde_json::to_string(&multi).unwrap())
            },
            StatusFormat::Svg => {
                let mut response = Response::new()
                    .with_header(ContentType("image/svg+xml;charset=utf-8".parse().unwrap()));
                if analysis_outcome.deps.any_outdated() {
                    response.set_body(assets::BADGE_OUTDATED_SVG.to_vec());
                } else {
                    response.set_body(assets::BADGE_UPTODATE_SVG.to_vec());
                }
                response
            }
        }
    }
}