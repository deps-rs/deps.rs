use std::{env, sync::Arc};

use futures::future;
use hyper::{
    header::{CONTENT_TYPE, LOCATION},
    Body, Error as HyperError, Method, Request, Response, StatusCode,
};
use once_cell::sync::Lazy;
use route_recognizer::{Params, Router};
use semver::VersionReq;
use slog::{error, o, Logger};

mod assets;
mod views;

use crate::engine::{AnalyzeDependenciesOutcome, Engine};
use crate::models::crates::{CrateName, CratePath};
use crate::models::repo::RepoPath;
use crate::models::SubjectPath;

#[derive(Debug, Clone, Copy, PartialEq)]
enum StatusFormat {
    Html,
    Svg,
}

#[derive(Debug, Clone, Copy)]
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
pub struct App {
    logger: Logger,
    engine: Engine,
    router: Arc<Router<Route>>,
}

impl App {
    pub fn new(logger: Logger, engine: Engine) -> App {
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

        App {
            logger,
            engine,
            router: Arc::new(router),
        }
    }

    pub async fn handle(&self, req: Request<Body>) -> Result<Response<Body>, HyperError> {
        let logger = self
            .logger
            .new(o!("http_path" => req.uri().path().to_owned()));

        if let Ok(route_match) = self.router.recognize(req.uri().path()) {
            match route_match.handler {
                &Route::Index => {
                    if *req.method() == Method::GET {
                        return self.index(req, route_match.params, logger).await;
                    }
                }
                &Route::RepoStatus(format) => {
                    if *req.method() == Method::GET {
                        return self
                            .repo_status(req, route_match.params, logger, format)
                            .await;
                    }
                }
                &Route::CrateStatus(format) => {
                    println!("route");
                    if *req.method() == Method::GET {
                        println!("get");
                        return self
                            .crate_status(req, route_match.params, logger, format)
                            .await;
                    }
                }
                &Route::CrateRedirect => {
                    if *req.method() == Method::GET {
                        return self.crate_redirect(req, route_match.params, logger).await;
                    }
                }
                &Route::Static(file) => {
                    if *req.method() == Method::GET {
                        return Ok(App::static_file(file));
                    }
                }
            }
        }

        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap())
    }
}

impl App {
    async fn index(
        &self,
        _req: Request<Body>,
        _params: Params,
        logger: Logger,
    ) -> Result<Response<Body>, HyperError> {
        let engine = self.engine.clone();

        let popular =
            future::try_join(engine.get_popular_repos(), engine.get_popular_crates()).await;

        match popular {
            Err(err) => {
                error!(logger, "error: {}", err);
                let mut response =
                    views::html::error::render("Could not retrieve popular items", "");
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                Ok(response)
            }
            Ok((popular_repos, popular_crates)) => {
                Ok(views::html::index::render(popular_repos, popular_crates))
            }
        }
    }

    async fn repo_status(
        &self,
        _req: Request<Body>,
        params: Params,
        logger: Logger,
        format: StatusFormat,
    ) -> Result<Response<Body>, HyperError> {
        let server = self.clone();

        let site = params.find("site").expect("route param 'site' not found");
        let qual = params.find("qual").expect("route param 'qual' not found");
        let name = params.find("name").expect("route param 'name' not found");

        let repo_path_result = RepoPath::from_parts(site, qual, name);

        match repo_path_result {
            Err(err) => {
                error!(logger, "error: {}", err);
                let mut response = views::html::error::render(
                    "Could not parse repository path",
                    "Please make sure to provide a valid repository path.",
                );
                *response.status_mut() = StatusCode::BAD_REQUEST;
                Ok(response)
            }

            Ok(repo_path) => {
                let analyze_result = server
                    .engine
                    .analyze_repo_dependencies(repo_path.clone())
                    .await;

                match analyze_result {
                    Err(err) => {
                        error!(logger, "error: {}", err);
                        let response =
                            App::status_format_analysis(None, format, SubjectPath::Repo(repo_path));
                        Ok(response)
                    }
                    Ok(analysis_outcome) => {
                        let response = App::status_format_analysis(
                            Some(analysis_outcome),
                            format,
                            SubjectPath::Repo(repo_path),
                        );
                        Ok(response)
                    }
                }
            }
        }
    }

    async fn crate_redirect(
        &self,
        _req: Request<Body>,
        params: Params,
        logger: Logger,
    ) -> Result<Response<Body>, HyperError> {
        let engine = self.engine.clone();

        let name = params.find("name").expect("route param 'name' not found");
        let crate_name_result = name.parse::<CrateName>();

        match crate_name_result {
            Err(err) => {
                error!(logger, "error: {}", err);
                let mut response = views::html::error::render(
                    "Could not parse crate name",
                    "Please make sure to provide a valid crate name.",
                );
                *response.status_mut() = StatusCode::BAD_REQUEST;
                Ok(response)
            }

            Ok(crate_name) => {
                let release_result = engine
                    .find_latest_crate_release(crate_name, VersionReq::any())
                    .await;

                match release_result {
                    Err(err) => {
                        error!(logger, "error: {}", err);
                        let mut response = views::html::error::render(
                            "Could not fetch crate information",
                            "Please make sure to provide a valid crate name.",
                        );
                        *response.status_mut() = StatusCode::NOT_FOUND;
                        Ok(response)
                    }
                    Ok(None) => {
                        let mut response = views::html::error::render(
                            "Could not fetch crate information",
                            "Please make sure to provide a valid crate name.",
                        );
                        *response.status_mut() = StatusCode::NOT_FOUND;
                        Ok(response)
                    }
                    Ok(Some(release)) => {
                        let redirect_url = format!(
                            "{}/crate/{}/{}",
                            &SELF_BASE_URL as &str,
                            release.name.as_ref(),
                            release.version
                        );

                        let res = Response::builder()
                            .status(StatusCode::TEMPORARY_REDIRECT)
                            .header(LOCATION, redirect_url)
                            .body(Body::empty())
                            .unwrap();

                        Ok(res)
                    }
                }
            }
        }
    }

    async fn crate_status(
        &self,
        _req: Request<Body>,
        params: Params,
        logger: Logger,
        format: StatusFormat,
    ) -> Result<Response<Body>, HyperError> {
        let server = self.clone();

        let name = params.find("name").expect("route param 'name' not found");
        let version = params
            .find("version")
            .expect("route param 'version' not found");

        let crate_path_result = CratePath::from_parts(name, version);

        println!("crate path {:?}", &crate_path_result);

        match crate_path_result {
            Err(err) => {
                error!(logger, "error: {}", err);
                let mut response = views::html::error::render(
                    "Could not parse crate path",
                    "Please make sure to provide a valid crate name and version.",
                );
                *response.status_mut() = StatusCode::BAD_REQUEST;
                Ok(response)
            }
            Ok(crate_path) => {
                println!("crate path ok");

                let analyze_result = server
                    .engine
                    .analyze_crate_dependencies(crate_path.clone())
                    .await;

                println!("results analyzed {:?}", &analyze_result);

                match analyze_result {
                    Err(err) => {
                        error!(logger, "error: {}", err);
                        let response = App::status_format_analysis(
                            None,
                            format,
                            SubjectPath::Crate(crate_path),
                        );
                        Ok(response)
                    }
                    Ok(analysis_outcome) => {
                        println!("analysis ok");

                        let response = App::status_format_analysis(
                            Some(analysis_outcome),
                            format,
                            SubjectPath::Crate(crate_path),
                        );

                        println!("response created");

                        Ok(response)
                    }
                }
            }
        }
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
