use std::sync::Arc;

use futures::{Future, IntoFuture, future};
use hyper::{Error as HyperError, Method, Request, Response, StatusCode};
use hyper::header::ContentType;
use route_recognizer::{Params, Router};
use slog::Logger;
use tokio_service::Service;

mod assets;
mod views;

use ::engine::{Engine, AnalyzeDependenciesOutcome};
use ::models::repo::RepoPath;

#[derive(Clone, Copy, PartialEq)]
enum StatusFormat {
    Html,
    Svg
}

#[derive(Clone, Copy)]
enum StaticFile {
    StyleCss,
    FaviconPng
}

enum Route {
    Index,
    Static(StaticFile),
    Status(StatusFormat)
}

#[derive(Clone)]
pub struct Server {
    logger: Logger,
    engine: Engine,
    router: Arc<Router<Route>>
}

impl Server {
    pub fn new(logger: Logger, engine: Engine) -> Server {
        let mut router = Router::new();

        router.add("/", Route::Index);

        router.add("/static/style.css", Route::Static(StaticFile::StyleCss));
        router.add("/static/favicon.png", Route::Static(StaticFile::FaviconPng));

        router.add("/repo/:site/:qual/:name", Route::Status(StatusFormat::Html));
        router.add("/repo/:site/:qual/:name/status.svg", Route::Status(StatusFormat::Svg));

        Server { logger, engine, router: Arc::new(router) }
    }
}

impl Service for Server {
    type Request = Request;
    type Response = Response;
    type Error = HyperError;
    type Future = Box<Future<Item=Response, Error=HyperError>>;

    fn call(&self, req: Request) -> Self::Future {
        let logger = self.logger.new(o!("http_path" => req.uri().path().to_owned()));

        if let Ok(route_match) = self.router.recognize(req.uri().path()) {
            match route_match.handler {
                &Route::Index => {
                    if *req.method() == Method::Get {
                        return Box::new(self.index(req, route_match.params, logger));
                    }
                },
                &Route::Status(format) => {
                    if *req.method() == Method::Get {
                        return Box::new(self.status(req, route_match.params, logger, format));
                    }
                },
                &Route::Static(file) => {
                    if *req.method() == Method::Get {
                        return Box::new(future::ok(Server::static_file(file)));
                    }
                }

            }
        }

        let mut response = Response::new();
        response.set_status(StatusCode::NotFound);
        Box::new(future::ok(response))
    }
}

impl Server {
    fn index(&self, _req: Request, _params: Params, logger: Logger) ->
        impl Future<Item=Response, Error=HyperError>
    {
        self.engine.get_popular_repos().then(move |popular_result| {
            match popular_result {
                Err(err) => {
                    error!(logger, "error: {}", err);
                    let mut response = views::html::error::render("Could not retrieve popular repositories", "");
                    response.set_status(StatusCode::InternalServerError);
                    future::ok(response)
                },
                Ok(popular) =>
                    future::ok(views::html::index::render(popular))
            }
        })
    }

    fn status(&self, _req: Request, params: Params, logger: Logger, format: StatusFormat) ->
        impl Future<Item=Response, Error=HyperError>
    {
        let server = self.clone();

        let site = params.find("site").expect("route param 'site' not found");
        let qual = params.find("qual").expect("route param 'qual' not found");
        let name = params.find("name").expect("route param 'name' not found");

        RepoPath::from_parts(site, qual, name).into_future().then(move |repo_path_result| {
            match repo_path_result {
                Err(err) => {
                    error!(logger, "error: {}", err);
                    let mut response = views::html::error::render("Could not parse repository path",
                        "Please make sure to provide a valid repository path.");
                    response.set_status(StatusCode::BadRequest);
                    future::Either::A(future::ok(response))
                },
                Ok(repo_path) => {
                    future::Either::B(server.engine.analyze_dependencies(repo_path.clone()).then(move |analyze_result| {
                        match analyze_result {
                            Err(err) => {
                                error!(logger, "error: {}", err);
                                let response = Server::status_format_analysis(None, format, repo_path);
                                future::ok(response)
                            },
                            Ok(analysis_outcome) => {
                                let response = Server::status_format_analysis(Some(analysis_outcome), format, repo_path);
                                future::ok(response)
                            }
                        }
                    }))
                }
            }
        })
    }

    fn status_format_analysis(analysis_outcome: Option<AnalyzeDependenciesOutcome>, format: StatusFormat, repo_path: RepoPath) -> Response {
        match format {
            StatusFormat::Svg =>
                views::badge::response(analysis_outcome.as_ref()),
            StatusFormat::Html =>
                views::html::status::render(analysis_outcome, repo_path)
        }
    }

    fn static_file(file: StaticFile) -> Response {
        match file {
            StaticFile::StyleCss => {
                Response::new()
                    .with_header(ContentType("text/css".parse().unwrap()))
                    .with_body(assets::STATIC_STYLE_CSS)
            },
            StaticFile::FaviconPng => {
                Response::new()
                    .with_header(ContentType("image/png".parse().unwrap()))
                    .with_body(assets::STATIC_FAVICON_PNG.to_vec())
            }
        }
    }
}
