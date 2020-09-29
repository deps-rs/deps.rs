use std::env;
use std::sync::Arc;

use futures::{future, Future, IntoFuture};
use hyper::header::{CONTENT_TYPE, LOCATION};
use hyper::service::Service;
use hyper::{Body, Error as HyperError, Method, Request, Response, StatusCode};
use once_cell::sync::Lazy;
use route_recognizer::{Params, Router};
use semver::VersionReq;
use slog::Logger;
use slog::{error, o};

mod assets;
mod views;

use crate::engine::{AnalyzeDependenciesOutcome, Engine};
use crate::models::crates::{CrateName, CratePath};
use crate::models::repo::RepoPath;
use crate::models::SubjectPath;

#[derive(Clone, Copy, PartialEq)]
enum StatusFormat {
    Html,
    Svg,
}

#[derive(Clone, Copy)]
enum StaticFile {
    StyleCss,
    FaviconPng,
}

enum Route {
    Index,
    Static(StaticFile),
    RepoStatus(StatusFormat),
    CrateRedirect,
    CrateStatus(StatusFormat),
}

#[derive(Clone)]
pub struct Server {
    logger: Logger,
    engine: Engine,
    router: Arc<Router<Route>>,
}

impl Server {
    pub fn new(logger: Logger, engine: Engine) -> Server {
        let mut router = Router::new();

        router.add("/", Route::Index);

        router.add("/static/style.css", Route::Static(StaticFile::StyleCss));
        router.add("/static/favicon.png", Route::Static(StaticFile::FaviconPng));

        router.add(
            "/repo/:site/:qual/:name",
            Route::RepoStatus(StatusFormat::Html),
        );
        router.add(
            "/repo/:site/:qual/:name/status.svg",
            Route::RepoStatus(StatusFormat::Svg),
        );

        router.add("/crate/:name", Route::CrateRedirect);
        router.add(
            "/crate/:name/:version",
            Route::CrateStatus(StatusFormat::Html),
        );
        router.add(
            "/crate/:name/:version/status.svg",
            Route::CrateStatus(StatusFormat::Svg),
        );

        Server {
            logger,
            engine,
            router: Arc::new(router),
        }
    }
}

impl Service for Server {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = hyper::Error;
    type Future = Box<dyn Future<Item = Response<Self::ResBody>, Error = Self::Error> + Send>;

    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
        let logger = self
            .logger
            .new(o!("http_path" => req.uri().path().to_owned()));

        if let Ok(route_match) = self.router.recognize(req.uri().path()) {
            match route_match.handler {
                &Route::Index => {
                    if *req.method() == Method::GET {
                        return Box::new(self.index(req, route_match.params, logger));
                    }
                }
                &Route::RepoStatus(format) => {
                    if *req.method() == Method::GET {
                        return Box::new(self.repo_status(req, route_match.params, logger, format));
                    }
                }
                &Route::CrateStatus(format) => {
                    if *req.method() == Method::GET {
                        return Box::new(self.crate_status(
                            req,
                            route_match.params,
                            logger,
                            format,
                        ));
                    }
                }
                &Route::CrateRedirect => {
                    if *req.method() == Method::GET {
                        return Box::new(self.crate_redirect(req, route_match.params, logger));
                    }
                }
                &Route::Static(file) => {
                    if *req.method() == Method::GET {
                        return Box::new(future::ok(Server::static_file(file)));
                    }
                }
            }
        }

        let mut response = Response::builder();
        response.status(StatusCode::NOT_FOUND);
        Box::new(future::ok(response.body(Body::empty()).unwrap()))
    }
}

impl Server {
    fn index(
        &self,
        _req: Request<Body>,
        _params: Params,
        logger: Logger,
    ) -> impl Future<Item = Response<Body>, Error = HyperError> + Send {
        self.engine
            .get_popular_repos()
            .join(self.engine.get_popular_crates())
            .then(move |popular_result| match popular_result {
                Err(err) => {
                    error!(logger, "error: {}", err);
                    let mut response =
                        views::html::error::render("Could not retrieve popular items", "");
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    future::ok(response)
                }
                Ok((popular_repos, popular_crates)) => {
                    future::ok(views::html::index::render(popular_repos, popular_crates))
                }
            })
    }

    fn repo_status(
        &self,
        _req: Request<Body>,
        params: Params,
        logger: Logger,
        format: StatusFormat,
    ) -> impl Future<Item = Response<Body>, Error = HyperError> + Send {
        let server = self.clone();

        let site = params.find("site").expect("route param 'site' not found");
        let qual = params.find("qual").expect("route param 'qual' not found");
        let name = params.find("name").expect("route param 'name' not found");

        RepoPath::from_parts(site, qual, name)
            .into_future()
            .then(move |repo_path_result| match repo_path_result {
                Err(err) => {
                    error!(logger, "error: {}", err);
                    let mut response = views::html::error::render(
                        "Could not parse repository path",
                        "Please make sure to provide a valid repository path.",
                    );
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    future::Either::A(future::ok(response))
                }
                Ok(repo_path) => future::Either::B(
                    server
                        .engine
                        .analyze_repo_dependencies(repo_path.clone())
                        .then(move |analyze_result| match analyze_result {
                            Err(err) => {
                                error!(logger, "error: {}", err);
                                let response = Server::status_format_analysis(
                                    None,
                                    format,
                                    SubjectPath::Repo(repo_path),
                                );
                                future::ok(response)
                            }
                            Ok(analysis_outcome) => {
                                let response = Server::status_format_analysis(
                                    Some(analysis_outcome),
                                    format,
                                    SubjectPath::Repo(repo_path),
                                );
                                future::ok(response)
                            }
                        }),
                ),
            })
    }

    fn crate_redirect(
        &self,
        _req: Request<Body>,
        params: Params,
        logger: Logger,
    ) -> impl Future<Item = Response<Body>, Error = HyperError> + Send {
        let engine = self.engine.clone();

        let name = params.find("name").expect("route param 'name' not found");

        name.parse::<CrateName>()
            .into_future()
            .then(move |crate_name_result| match crate_name_result {
                Err(err) => {
                    error!(logger, "error: {}", err);
                    let mut response = views::html::error::render(
                        "Could not parse crate name",
                        "Please make sure to provide a valid crate name.",
                    );
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    future::Either::A(future::ok(response))
                }
                Ok(crate_name) => future::Either::B(
                    engine
                        .find_latest_crate_release(crate_name, VersionReq::any())
                        .then(move |release_result| match release_result {
                            Err(err) => {
                                error!(logger, "error: {}", err);
                                let mut response = views::html::error::render(
                                    "Could not fetch crate information",
                                    "Please make sure to provide a valid crate name.",
                                );
                                *response.status_mut() = StatusCode::NOT_FOUND;
                                future::ok(response)
                            }
                            Ok(None) => {
                                let mut response = views::html::error::render(
                                    "Could not fetch crate information",
                                    "Please make sure to provide a valid crate name.",
                                );
                                *response.status_mut() = StatusCode::NOT_FOUND;
                                future::ok(response)
                            }
                            Ok(Some(release)) => {
                                let mut response = Response::builder();
                                response.status(StatusCode::TEMPORARY_REDIRECT);
                                let url = format!(
                                    "{}/crate/{}/{}",
                                    &SELF_BASE_URL as &str,
                                    release.name.as_ref(),
                                    release.version
                                );
                                response.header(LOCATION, url);

                                let response = response.body(Body::empty()).unwrap();
                                future::ok(response)
                            }
                        }),
                ),
            })
    }

    fn crate_status(
        &self,
        _req: Request<Body>,
        params: Params,
        logger: Logger,
        format: StatusFormat,
    ) -> impl Future<Item = Response<Body>, Error = HyperError> + Send {
        let server = self.clone();

        let name = params.find("name").expect("route param 'name' not found");
        let version = params
            .find("version")
            .expect("route param 'version' not found");

        CratePath::from_parts(name, version)
            .into_future()
            .then(move |crate_path_result| match crate_path_result {
                Err(err) => {
                    error!(logger, "error: {}", err);
                    let mut response = views::html::error::render(
                        "Could not parse crate path",
                        "Please make sure to provide a valid crate name and version.",
                    );
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    future::Either::A(future::ok(response))
                }
                Ok(crate_path) => future::Either::B(
                    server
                        .engine
                        .analyze_crate_dependencies(crate_path.clone())
                        .then(move |analyze_result| match analyze_result {
                            Err(err) => {
                                error!(logger, "error: {}", err);
                                let response = Server::status_format_analysis(
                                    None,
                                    format,
                                    SubjectPath::Crate(crate_path),
                                );
                                future::ok(response)
                            }
                            Ok(analysis_outcome) => {
                                let response = Server::status_format_analysis(
                                    Some(analysis_outcome),
                                    format,
                                    SubjectPath::Crate(crate_path),
                                );
                                future::ok(response)
                            }
                        }),
                ),
            })
    }

    fn status_format_analysis(
        analysis_outcome: Option<AnalyzeDependenciesOutcome>,
        format: StatusFormat,
        subject_path: SubjectPath,
    ) -> Response<Body> {
        match format {
            StatusFormat::Svg => views::badge::response(analysis_outcome.as_ref()),
            StatusFormat::Html => views::html::status::render(analysis_outcome, subject_path),
        }
    }

    fn static_file(file: StaticFile) -> Response<Body> {
        match file {
            StaticFile::StyleCss => Response::builder()
                .header(CONTENT_TYPE, "text/css")
                .body(Body::from(assets::STATIC_STYLE_CSS))
                .unwrap(),
            StaticFile::FaviconPng => Response::builder()
                .header(CONTENT_TYPE, "image/png")
                .body(Body::from(assets::STATIC_FAVICON_PNG))
                .unwrap(),
        }
    }
}

static SELF_BASE_URL: Lazy<String> =
    Lazy::new(|| env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string()));
