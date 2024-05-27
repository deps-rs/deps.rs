use std::{env, sync::Arc, time::Instant};

use actix_http::{
    body::MessageBody,
    header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, LOCATION},
    Method, Request, Response, StatusCode,
};
use badge::BadgeStyle;
use futures_util::future;
use once_cell::sync::Lazy;
use route_recognizer::{Params, Router};
use semver::VersionReq;
use serde::Deserialize;

mod assets;
mod views;

use self::assets::{
    STATIC_LINKS_JS_ETAG, STATIC_LINKS_JS_PATH, STATIC_STYLE_CSS_ETAG, STATIC_STYLE_CSS_PATH,
};
use crate::{
    engine::{AnalyzeDependenciesOutcome, Engine},
    models::{
        crates::{CrateName, CratePath},
        repo::RepoPath,
        SubjectPath,
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum StatusFormat {
    Html,
    Svg,
}

#[derive(Debug, Clone, Copy)]
enum StaticFile {
    StyleCss,
    FaviconPng,
    LinksJs,
}

enum Route {
    Index,
    Static(StaticFile),
    RepoStatus(StatusFormat),
    CrateRedirect,
    CrateStatus(StatusFormat),
    LatestCrateBadge,
}

#[derive(Clone)]
pub struct App {
    engine: Engine,
    router: Arc<Router<Route>>,
}

impl App {
    pub fn new(engine: Engine) -> App {
        let mut router = Router::new();

        router.add("/", Route::Index);

        router.add(STATIC_STYLE_CSS_PATH, Route::Static(StaticFile::StyleCss));
        router.add("/static/logo.svg", Route::Static(StaticFile::FaviconPng));
        router.add(STATIC_LINKS_JS_PATH, Route::Static(StaticFile::LinksJs));

        router.add(
            "/repo/*site/:qual/:name",
            Route::RepoStatus(StatusFormat::Html),
        );
        router.add(
            "/repo/*site/:qual/:name/status.svg",
            Route::RepoStatus(StatusFormat::Svg),
        );

        router.add("/crate/:name", Route::CrateRedirect);
        router.add(
            "/crate/:name/:version",
            Route::CrateStatus(StatusFormat::Html),
        );
        router.add("/crate/:name/latest/status.svg", Route::LatestCrateBadge);
        router.add(
            "/crate/:name/:version/status.svg",
            Route::CrateStatus(StatusFormat::Svg),
        );

        App {
            engine,
            router: Arc::new(router),
        }
    }

    pub async fn handle(
        &self,
        req: Request,
    ) -> Result<Response<impl MessageBody>, actix_http::Error> {
        let start = Instant::now();

        // allows `/path/` to also match `/path`
        let normalized_path = req.uri().path().trim_end_matches('/');

        let res = if let Ok(route_match) = self.router.recognize(normalized_path) {
            match (req.method(), route_match.handler()) {
                (&Method::GET, Route::Index) => self
                    .index(req, route_match.params().clone())
                    .await
                    .map(Response::map_into_boxed_body),

                (&Method::GET, Route::RepoStatus(format)) => self
                    .repo_status(req, route_match.params().clone(), *format)
                    .await
                    .map(Response::map_into_boxed_body),

                (&Method::GET, Route::CrateStatus(format)) => self
                    .crate_status(req, route_match.params().clone(), *format)
                    .await
                    .map(Response::map_into_boxed_body),

                (&Method::GET, Route::LatestCrateBadge) => self
                    .crate_status(req, route_match.params().clone(), StatusFormat::Svg)
                    .await
                    .map(Response::map_into_boxed_body),

                (&Method::GET, Route::CrateRedirect) => self
                    .crate_redirect(req, route_match.params().clone())
                    .await
                    .map(Response::map_into_boxed_body),

                (&Method::GET, Route::Static(file)) => {
                    Ok(App::static_file(*file).map_into_boxed_body())
                }

                _ => Ok(not_found().map_into_boxed_body()),
            }
        } else {
            Ok(not_found().map_into_boxed_body())
        };

        let end = Instant::now();
        let diff = end - start;

        match &res {
            Ok(res) => tracing::info!(
                status = %res.status(),
                time = %format_args!("{}ms", diff.as_millis()),
            ),
            Err(err) => tracing::error!(%err),
        };

        res
    }
}

impl App {
    async fn index(
        &self,
        _req: Request,
        _params: Params,
    ) -> Result<Response<impl MessageBody>, actix_http::Error> {
        let engine = self.engine.clone();

        let popular =
            future::try_join(engine.get_popular_repos(), engine.get_popular_crates()).await;

        match popular {
            Err(err) => {
                tracing::error!(%err);
                let mut response =
                    views::html::error::render("Could not retrieve popular items", "");
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;

                Ok(response.map_into_boxed_body())
            }
            Ok((popular_repos, popular_crates)) => {
                Ok(views::html::index::render(popular_repos, popular_crates).map_into_boxed_body())
            }
        }
    }

    async fn repo_status(
        &self,
        req: Request,
        params: Params,
        format: StatusFormat,
    ) -> Result<Response<impl MessageBody>, actix_http::Error> {
        let server = self.clone();

        let site = params.find("site").expect("route param 'site' not found");
        let qual = params.find("qual").expect("route param 'qual' not found");
        let name = params.find("name").expect("route param 'name' not found");

        let extra_knobs = ExtraConfig::from_query_string(req.uri().query());

        let repo_path_result = RepoPath::from_parts(site, qual, name);

        match repo_path_result {
            Err(err) => {
                tracing::error!(%err);
                let mut response = views::html::error::render(
                    "Could not parse repository path",
                    "Please make sure to provide a valid repository path.",
                );
                *response.status_mut() = StatusCode::BAD_REQUEST;

                Ok(response.map_into_boxed_body())
            }

            Ok(repo_path) => {
                let analyze_result = server
                    .engine
                    .analyze_repo_dependencies(repo_path.clone(), &extra_knobs.path)
                    .await;

                match analyze_result {
                    Err(err) => {
                        tracing::error!(%err);
                        let response = App::status_format_analysis(
                            None,
                            format,
                            SubjectPath::Repo(repo_path),
                            extra_knobs,
                        );

                        Ok(response.map_into_boxed_body())
                    }
                    Ok(analysis_outcome) => {
                        let response = App::status_format_analysis(
                            Some(analysis_outcome),
                            format,
                            SubjectPath::Repo(repo_path),
                            extra_knobs,
                        );

                        Ok(response.map_into_boxed_body())
                    }
                }
            }
        }
    }

    async fn crate_redirect(
        &self,
        _req: Request,
        params: Params,
    ) -> Result<Response<impl MessageBody>, actix_http::Error> {
        let engine = self.engine.clone();

        let name = params.find("name").expect("route param 'name' not found");
        let crate_name_result = name.parse::<CrateName>();

        match crate_name_result {
            Err(err) => {
                tracing::error!(%err);
                let mut response = views::html::error::render(
                    "Could not parse crate name",
                    "Please make sure to provide a valid crate name.",
                );
                *response.status_mut() = StatusCode::BAD_REQUEST;

                Ok(response.map_into_boxed_body())
            }

            Ok(crate_name) => {
                let release_result = engine
                    .find_latest_stable_crate_release(crate_name, VersionReq::STAR)
                    .await;

                match release_result {
                    Err(err) => {
                        tracing::error!(%err);
                        let mut response = views::html::error::render(
                            "Could not fetch crate information",
                            "Please make sure to provide a valid crate name.",
                        );
                        *response.status_mut() = StatusCode::NOT_FOUND;

                        Ok(response.map_into_boxed_body())
                    }
                    Ok(None) => {
                        let mut response = views::html::error::render(
                            "Could not fetch crate information",
                            "Please make sure to provide a valid crate name.",
                        );
                        *response.status_mut() = StatusCode::NOT_FOUND;

                        Ok(response.map_into_boxed_body())
                    }
                    Ok(Some(release)) => {
                        let redirect_url = format!(
                            "{}/crate/{}/{}",
                            &SELF_BASE_URL as &str,
                            release.name.as_ref(),
                            release.version
                        );

                        let res = Response::build(StatusCode::TEMPORARY_REDIRECT)
                            .insert_header((LOCATION, redirect_url))
                            .finish();

                        Ok(res.map_into_boxed_body())
                    }
                }
            }
        }
    }

    async fn crate_status(
        &self,
        req: Request,
        params: Params,
        format: StatusFormat,
    ) -> Result<Response<impl MessageBody>, actix_http::Error> {
        let server = self.clone();

        let name = params.find("name").expect("route param 'name' not found");

        let version = match params.find("version") {
            Some(ver) => ver.to_owned(),
            None => {
                let crate_name = match name.parse() {
                    Ok(name) => name,
                    Err(_) => {
                        let mut response = views::html::error::render(
                            "Could not parse crate path",
                            "Please make sure to provide a valid crate name and version.",
                        );
                        *response.status_mut() = StatusCode::BAD_REQUEST;

                        return Ok(response.map_into_boxed_body());
                    }
                };

                match server
                    .engine
                    .find_latest_stable_crate_release(crate_name, VersionReq::STAR)
                    .await
                {
                    Ok(Some(latest_rel)) => latest_rel.version.to_string(),
                    Ok(None) => return Ok(not_found().map_into_boxed_body()),
                    Err(err) => {
                        tracing::error!(%err);
                        let mut response = views::html::error::render(
                            "Could not fetch crate information",
                            "Please make sure to provide a valid crate name.",
                        );
                        *response.status_mut() = StatusCode::NOT_FOUND;

                        return Ok(response.map_into_boxed_body());
                    }
                }
            }
        };

        let crate_path_result = CratePath::from_parts(name, &version);
        let badge_knobs = ExtraConfig::from_query_string(req.uri().query());

        match crate_path_result {
            Err(err) => {
                tracing::error!(%err);
                let mut response = views::html::error::render(
                    "Could not parse crate path",
                    "Please make sure to provide a valid crate name and version.",
                );
                *response.status_mut() = StatusCode::BAD_REQUEST;

                Ok(response.map_into_boxed_body())
            }

            Ok(crate_path) => {
                let analyze_result = server
                    .engine
                    .analyze_crate_dependencies(crate_path.clone())
                    .await;

                match analyze_result {
                    Err(err) => {
                        tracing::error!(%err);
                        let response = App::status_format_analysis(
                            None,
                            format,
                            SubjectPath::Crate(crate_path),
                            badge_knobs,
                        );

                        Ok(response.map_into_boxed_body())
                    }
                    Ok(analysis_outcome) => {
                        let response = App::status_format_analysis(
                            Some(analysis_outcome),
                            format,
                            SubjectPath::Crate(crate_path),
                            badge_knobs,
                        );

                        Ok(response.map_into_boxed_body())
                    }
                }
            }
        }
    }

    fn status_format_analysis(
        analysis_outcome: Option<AnalyzeDependenciesOutcome>,
        format: StatusFormat,
        subject_path: SubjectPath,
        badge_knobs: ExtraConfig,
    ) -> Response<impl MessageBody> {
        match format {
            StatusFormat::Svg => {
                views::badge::response(analysis_outcome.as_ref(), badge_knobs).map_into_boxed_body()
            }

            StatusFormat::Html => {
                views::html::status::render(analysis_outcome, subject_path, badge_knobs)
                    .map_into_boxed_body()
            }
        }
    }

    fn static_file(file: StaticFile) -> Response<impl MessageBody> {
        match file {
            StaticFile::StyleCss => Response::build(StatusCode::OK)
                .insert_header((CONTENT_TYPE, "text/css; charset=utf-8"))
                .insert_header((ETAG, STATIC_STYLE_CSS_ETAG))
                .insert_header((CACHE_CONTROL, "public, max-age=365000000, immutable"))
                .body(assets::STATIC_STYLE_CSS),

            StaticFile::FaviconPng => Response::build(StatusCode::OK)
                .insert_header((CONTENT_TYPE, "image/svg+xml"))
                .body(assets::STATIC_FAVICON),

            StaticFile::LinksJs => Response::build(StatusCode::OK)
                .insert_header((CONTENT_TYPE, "text/javascript; charset=utf-8"))
                .insert_header((ETAG, STATIC_LINKS_JS_ETAG))
                .insert_header((CACHE_CONTROL, "public, max-age=365000000, immutable"))
                .body(assets::STATIC_LINKS_JS),
        }
    }
}

fn not_found() -> Response<impl MessageBody> {
    views::html::error::render_404()
}

static SELF_BASE_URL: Lazy<String> =
    Lazy::new(|| env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string()));

/// Configuration options supplied through Get Parameters
#[derive(Debug, Clone, Default)]
pub struct ExtraConfig {
    /// Badge style to show
    style: BadgeStyle,
    /// Whether the inscription _"dependencies"_ should be abbreviated as _"deps"_ in the badge.
    compact: bool,
    /// Path in which the crate resides within the repository
    path: Option<String>,
}

impl ExtraConfig {
    fn from_query_string(qs: Option<&str>) -> Self {
        #[derive(Debug, Clone, Default, Deserialize)]
        struct ExtraConfigPartial {
            style: Option<BadgeStyle>,
            compact: Option<bool>,
            path: Option<String>,
        }

        let extra_config = qs
            .and_then(|qs| serde_urlencoded::from_str::<ExtraConfigPartial>(qs).ok())
            .unwrap_or_default();

        Self {
            style: extra_config.style.unwrap_or_default(),
            compact: extra_config.compact.unwrap_or_default(),
            path: extra_config.path,
        }
    }
}
