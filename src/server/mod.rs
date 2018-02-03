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
    Json,
    Svg
}

#[derive(Clone, Copy)]
enum StaticFile {
    StyleCss
}

enum Route {
    Index,
    Static(StaticFile),
    Status(StatusFormat)
}

#[derive(Clone)]
pub struct Server {
    engine: Engine,
    router: Arc<Router<Route>>
}

impl Server {
    pub fn new(engine: Engine) -> Server {
        let mut router = Router::new();

        router.add("/", Route::Index);

        router.add("/static/style.css", Route::Static(StaticFile::StyleCss));

        router.add("/repo/:site/:qual/:name", Route::Status(StatusFormat::Html));
        router.add("/repo/:site/:qual/:name/status.json", Route::Status(StatusFormat::Json));
        router.add("/repo/:site/:qual/:name/status.svg", Route::Status(StatusFormat::Svg));

        Server { engine, router: Arc::new(router) }
    }
}

impl Service for Server {
    type Request = Request;
    type Response = Response;
    type Error = HyperError;
    type Future = Box<Future<Item=Response, Error=HyperError>>;

    fn call(&self, req: Request) -> Self::Future {
        if let Ok(route_match) = self.router.recognize(req.uri().path()) {
            match route_match.handler {
                &Route::Index => {
                    if *req.method() == Method::Get {
                        return Box::new(self.index(req, route_match.params));
                    }
                },
                &Route::Status(format) => {
                    if *req.method() == Method::Get {
                        return Box::new(self.status(req, route_match.params, format));
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
    fn index(&self, _req: Request, _params: Params) ->
        impl Future<Item=Response, Error=HyperError>
    {
        self.engine.get_popular_repos().then(|popular_result| {
            match popular_result {
                Err(err) => {
                    let mut response = Response::new();
                    response.set_status(StatusCode::BadRequest);
                    response.set_body(format!("{:?}", err));
                    future::ok(response)
                },
                Ok(popular) =>
                    future::ok(views::html::index::render(popular))
            }
        })
    }

    fn status(&self, _req: Request, params: Params, format: StatusFormat) ->
        impl Future<Item=Response, Error=HyperError>
    {
        let server = self.clone();

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
                    future::Either::B(server.engine.analyze_dependencies(repo_path.clone()).then(move |analyze_result| {
                        match analyze_result {
                            Err(err) => {
                                if format != StatusFormat::Svg {
                                    let mut response = Response::new();
                                    response.set_status(StatusCode::BadRequest);
                                    response.set_body(format!("{:?}", err));
                                    future::Either::A(future::ok(response))
                                } else {
                                    future::Either::A(future::ok(views::status_svg(None)))
                                }
                            },
                            Ok(analysis_outcome) => {
                                let response = Server::status_format_analysis(analysis_outcome, format, repo_path);
                                future::Either::B(future::ok(response))
                            }
                        }
                    }))
                }
            }
        })
    }

    fn status_format_analysis(analysis_outcome: AnalyzeDependenciesOutcome, format: StatusFormat, repo_path: RepoPath) -> Response {
        match format {
            StatusFormat::Json =>
                views::status_json(analysis_outcome),
            StatusFormat::Svg =>
                views::status_svg(Some(analysis_outcome)),
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
            }
        }
    }
}
