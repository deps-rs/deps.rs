use std::env;

use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG},
        StatusCode,
    },
    response::{IntoResponse as _, Redirect, Response},
    routing::get,
    Router,
};
use badge::BadgeStyle;
use futures_util::future;
use once_cell::sync::Lazy;
use semver::VersionReq;
use serde::Deserialize;
use tower_http::{normalize_path::NormalizePathLayer, trace::TraceLayer};

mod assets;
mod views;

use self::assets::{
    STATIC_FAVICON_PATH, STATIC_LINKS_JS_ETAG, STATIC_LINKS_JS_PATH, STATIC_STYLE_CSS_ETAG,
    STATIC_STYLE_CSS_PATH,
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

#[derive(Clone)]
pub struct App {
    engine: Engine,
}

impl App {
    pub fn new(engine: Engine) -> App {
        App { engine }
    }

    pub(crate) fn router() -> Router<App> {
        Router::new()
            .route("/", get(App::index))
            .route("/crate/:name", get(App::crate_redirect))
            .route("/crate/:name/:version", get(App::crate_status_html))
            .route(
                "/crate/:name/latest/status.svg",
                get(App::crate_latest_status_svg),
            )
            .route(
                "/crate/:name/:version/status.svg",
                get(App::crate_status_svg),
            )
            // TODO: `:site` isn't quite right, original was `*site`
            .route("/repo/:site/:qual/:name", get(App::repo_status_html))
            .route(
                "/repo/:site/:qual/:name/status.svg",
                get(App::repo_status_svg),
            )
            .route(
                STATIC_STYLE_CSS_PATH,
                get(|| App::static_file(StaticFile::StyleCss)),
            )
            .route(
                STATIC_FAVICON_PATH,
                get(|| App::static_file(StaticFile::FaviconPng)),
            )
            .route(
                STATIC_LINKS_JS_PATH,
                get(|| App::static_file(StaticFile::LinksJs)),
            )
            .fallback(|| async { not_found() })
            .layer(NormalizePathLayer::trim_trailing_slash())
            .layer(TraceLayer::new_for_http())
    }

    async fn index(State(app): State<App>) -> Response {
        let engine = app.engine.clone();

        let popular =
            future::try_join(engine.get_popular_repos(), engine.get_popular_crates()).await;

        match popular {
            Err(err) => {
                tracing::error!(%err);
                let mut response =
                    views::html::error::render("Could not retrieve popular items", "");
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                response
            }

            Ok((popular_repos, popular_crates)) => {
                views::html::index::render(popular_repos, popular_crates)
            }
        }
    }

    async fn repo_status_html(
        State(app): State<App>,
        Path(params): Path<(String, String, String)>,
        req: Request,
    ) -> Response {
        Self::repo_status(app, params, req, StatusFormat::Html).await
    }

    async fn repo_status_svg(
        State(app): State<App>,
        Path(params): Path<(String, String, String)>,
        req: Request,
    ) -> Response {
        Self::repo_status(app, params, req, StatusFormat::Svg).await
    }

    async fn repo_status(
        app: App,
        (site, qual, name): (String, String, String),
        req: Request,
        format: StatusFormat,
    ) -> Response {
        let engine = app.engine.clone();

        let extra_knobs = ExtraConfig::from_query_string(req.uri().query());

        let repo_path_result = RepoPath::from_parts(&site, &qual, &name);

        match repo_path_result {
            Err(err) => {
                tracing::error!(%err);
                let mut response = views::html::error::render(
                    "Could not parse repository path",
                    "Please make sure to provide a valid repository path.",
                );
                *response.status_mut() = StatusCode::BAD_REQUEST;
                response
            }

            Ok(repo_path) => {
                let analyze_result = engine
                    .analyze_repo_dependencies(repo_path.clone(), &extra_knobs.path)
                    .await;

                match analyze_result {
                    Err(err) => {
                        tracing::error!(%err);

                        App::status_format_analysis(
                            None,
                            format,
                            SubjectPath::Repo(repo_path),
                            extra_knobs,
                        )
                    }

                    Ok(analysis_outcome) => App::status_format_analysis(
                        Some(analysis_outcome),
                        format,
                        SubjectPath::Repo(repo_path),
                        extra_knobs,
                    ),
                }
            }
        }
    }

    async fn crate_redirect(State(app): State<App>, Path((name,)): Path<(String,)>) -> Response {
        let engine = app.engine.clone();

        let crate_name_result = name.parse::<CrateName>();

        match crate_name_result {
            Err(err) => {
                tracing::error!(%err);
                let mut response = views::html::error::render(
                    "Could not parse crate name",
                    "Please make sure to provide a valid crate name.",
                );
                *response.status_mut() = StatusCode::BAD_REQUEST;
                response
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
                        response
                    }
                    Ok(None) => {
                        let mut response = views::html::error::render(
                            "Could not fetch crate information",
                            "Please make sure to provide a valid crate name.",
                        );
                        *response.status_mut() = StatusCode::NOT_FOUND;
                        response
                    }
                    Ok(Some(release)) => {
                        let redirect_url = format!(
                            "{}/crate/{}/{}",
                            &SELF_BASE_URL as &str,
                            release.name.as_ref(),
                            release.version
                        );

                        Redirect::temporary(&redirect_url).into_response()
                    }
                }
            }
        }
    }

    async fn crate_status_html(
        State(app): State<App>,
        Path((name, version)): Path<(String, String)>,
        req: Request,
    ) -> Response {
        Self::crate_status(app, (name, Some(version)), req, StatusFormat::Html).await
    }

    async fn crate_status_svg(
        State(app): State<App>,
        Path((name, version)): Path<(String, String)>,
        req: Request,
    ) -> Response {
        Self::crate_status(app, (name, Some(version)), req, StatusFormat::Svg).await
    }

    async fn crate_latest_status_svg(
        State(app): State<App>,
        Path((name,)): Path<(String,)>,
        req: Request,
    ) -> Response {
        Self::crate_status(app, (name, None), req, StatusFormat::Svg).await
    }

    async fn crate_status(
        app: App,
        (name, version): (String, Option<String>),
        req: Request,
        format: StatusFormat,
    ) -> Response {
        let server = app.clone();

        let version = match version {
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
                        return response;
                    }
                };

                match server
                    .engine
                    .find_latest_stable_crate_release(crate_name, VersionReq::STAR)
                    .await
                {
                    Ok(Some(latest_rel)) => latest_rel.version.to_string(),
                    Ok(None) => return not_found(),
                    Err(err) => {
                        tracing::error!(%err);
                        let mut response = views::html::error::render(
                            "Could not fetch crate information",
                            "Please make sure to provide a valid crate name.",
                        );
                        *response.status_mut() = StatusCode::NOT_FOUND;
                        return response;
                    }
                }
            }
        };

        let crate_path_result = CratePath::from_parts(&name, &version);
        let badge_knobs = ExtraConfig::from_query_string(req.uri().query());

        match crate_path_result {
            Err(err) => {
                tracing::error!(%err);
                let mut response = views::html::error::render(
                    "Could not parse crate path",
                    "Please make sure to provide a valid crate name and version.",
                );
                *response.status_mut() = StatusCode::BAD_REQUEST;
                response
            }

            Ok(crate_path) => {
                let analyze_result = server
                    .engine
                    .analyze_crate_dependencies(crate_path.clone())
                    .await;

                match analyze_result {
                    Err(err) => {
                        tracing::error!(%err);
                        App::status_format_analysis(
                            None,
                            format,
                            SubjectPath::Crate(crate_path),
                            badge_knobs,
                        )
                    }

                    Ok(analysis_outcome) => App::status_format_analysis(
                        Some(analysis_outcome),
                        format,
                        SubjectPath::Crate(crate_path),
                        badge_knobs,
                    ),
                }
            }
        }
    }

    fn status_format_analysis(
        analysis_outcome: Option<AnalyzeDependenciesOutcome>,
        format: StatusFormat,
        subject_path: SubjectPath,
        badge_knobs: ExtraConfig,
    ) -> Response {
        match format {
            StatusFormat::Svg => views::badge::response(analysis_outcome.as_ref(), badge_knobs),
            StatusFormat::Html => {
                views::html::status::render(analysis_outcome, subject_path, badge_knobs)
            }
        }
    }

    async fn static_file(file: StaticFile) -> Response {
        match file {
            StaticFile::StyleCss => Response::builder()
                .header(CONTENT_TYPE, "text/css; charset=utf-8")
                .header(ETAG, STATIC_STYLE_CSS_ETAG)
                .header(CACHE_CONTROL, "public, max-age=365000000, immutable")
                .body(Body::from(assets::STATIC_STYLE_CSS))
                .unwrap(),

            StaticFile::FaviconPng => Response::builder()
                .header(CONTENT_TYPE, "image/svg+xml")
                .body(Body::from(assets::STATIC_FAVICON))
                .unwrap(),

            StaticFile::LinksJs => Response::builder()
                .header(CONTENT_TYPE, "text/javascript; charset=utf-8")
                .header(ETAG, STATIC_LINKS_JS_ETAG)
                .header(CACHE_CONTROL, "public, max-age=365000000, immutable")
                .body(Body::from(assets::STATIC_LINKS_JS))
                .unwrap(),
        }
    }
}

fn not_found() -> Response {
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
