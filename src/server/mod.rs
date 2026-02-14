use std::{env, sync::LazyLock};

use actix_web::{
    Either, HttpResponse, Resource, Responder, get,
    http::{
        StatusCode, Uri,
        header::{ContentType, ETag, EntityTag},
    },
    web::{Html, Redirect, ServiceConfig, ThinData},
};
use actix_web_lab::{
    extract::Path,
    header::{CacheControl, CacheDirective},
};
use assets::STATIC_FAVICON_PATH;
use badge::BadgeStyle;
use futures_util::future;
use semver::VersionReq;
use serde::Deserialize;

mod assets;
mod error;
mod views;

use self::{
    assets::{
        STATIC_LINKS_JS_ETAG, STATIC_LINKS_JS_PATH, STATIC_STYLE_CSS_ETAG, STATIC_STYLE_CSS_PATH,
    },
    error::ServerError,
};
use crate::{
    engine::{AnalyzeDependenciesOutcome, Engine},
    models::{
        SubjectPath,
        crates::{CrateName, CratePath},
        repo::RepoPath,
    },
    utils::common::{UntaggedEither, WrappedBool, safe_truncate},
};

const MAX_SUBJECT_WIDTH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq)]
enum StatusFormat {
    Html,
    Svg,
    /// Renders the analysis status as a JSON object compatible with the shields.io endpoint badge.
    /// See: https://shields.io/badges/endpoint-badge
    ShieldJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeTabMode {
    Hidden,
    PinnedDefault,
    LatestDefault,
}

#[get("/")]
pub(crate) async fn index(ThinData(engine): ThinData<Engine>) -> actix_web::Result<impl Responder> {
    let popular = future::try_join(engine.get_popular_repos(), engine.get_popular_crates()).await;

    match popular {
        Err(err) => {
            tracing::error!(%err);
            Err(ServerError::PopularItemsFailed.into())
        }
        Ok((popular_repos, popular_crates)) => Ok(Html::new(
            views::html::index::render(popular_repos, popular_crates).0,
        )),
    }
}

#[get("/repo/{site:.+?}/{qual}/{name}/status.svg")]
pub(crate) async fn repo_status_svg(
    ThinData(engine): ThinData<Engine>,
    uri: Uri,
    Path(params): Path<(String, String, String)>,
) -> actix_web::Result<impl Responder> {
    repo_status(engine, uri, params, StatusFormat::Svg).await
}

#[get("/repo/{site:.+?}/{qual}/{name}/shield.json")]
pub(crate) async fn repo_status_shield_json(
    ThinData(engine): ThinData<Engine>,
    uri: Uri,
    Path(params): Path<(String, String, String)>,
) -> actix_web::Result<impl Responder> {
    repo_status(engine, uri, params, StatusFormat::ShieldJson).await
}

#[get("/repo/{site:.+?}/{qual}/{name}")]
pub(crate) async fn repo_status_html(
    ThinData(engine): ThinData<Engine>,
    uri: Uri,
    Path(params): Path<(String, String, String)>,
) -> actix_web::Result<impl Responder> {
    repo_status(engine, uri, params, StatusFormat::Html).await
}

async fn repo_status(
    engine: Engine,
    uri: Uri,
    (site, qual, name): (String, String, String),
    format: StatusFormat,
) -> actix_web::Result<impl Responder> {
    let extra_knobs = ExtraConfig::from_query_string(uri.query());

    let repo_path_result = RepoPath::from_parts(&site, &qual, &name);

    let repo_path = match repo_path_result {
        Ok(repo_path) => repo_path,
        Err(err) => {
            tracing::error!(%err);
            return Err(ServerError::BadRepoPath.into());
        }
    };

    let analyze_result = engine
        .analyze_repo_dependencies(repo_path.clone(), &extra_knobs.path)
        .await;

    match analyze_result {
        Err(err) => {
            tracing::error!(%err);
            let response = status_format_analysis(
                None,
                format,
                SubjectPath::Repo(repo_path),
                extra_knobs,
                BadgeTabMode::Hidden,
            );

            Ok(response)
        }

        Ok(analysis_outcome) => {
            let response = status_format_analysis(
                Some(analysis_outcome),
                format,
                SubjectPath::Repo(repo_path),
                extra_knobs,
                BadgeTabMode::Hidden,
            );

            Ok(response)
        }
    }
}

#[get("/crate/{name}")]
async fn crate_redirect(
    ThinData(engine): ThinData<Engine>,
    Path((name,)): Path<(String,)>,
) -> actix_web::Result<impl Responder> {
    let crate_name_result = name.parse::<CrateName>();

    let crate_name = match crate_name_result {
        Ok(crate_name) => crate_name,
        Err(err) => {
            tracing::error!(%err);
            return Err(ServerError::BadCratePath.into());
        }
    };

    let release_result = engine
        .find_latest_stable_crate_release(crate_name, VersionReq::STAR)
        .await
        .inspect_err(|err| {
            tracing::error!(%err);
        });

    let Ok(Some(release)) = release_result else {
        return Err(ServerError::CrateFetchFailed.into());
    };

    let redirect_url = format!(
        "{}/crate/{}/{}",
        &SELF_BASE_URL as &str,
        release.name.as_ref(),
        release.version
    );

    Ok(Redirect::to(redirect_url))
}

#[get("/crate/{name}/{version}")]
async fn crate_status_html(
    ThinData(engine): ThinData<Engine>,
    uri: Uri,
    Path((name, version)): Path<(String, String)>,
) -> actix_web::Result<impl Responder> {
    crate_status(engine, uri, (name, Some(version)), StatusFormat::Html).await
}

#[get("/crate/{name}/latest")]
async fn crate_latest_status_html(
    ThinData(engine): ThinData<Engine>,
    uri: Uri,
    Path((name,)): Path<(String,)>,
) -> actix_web::Result<impl Responder> {
    crate_status(engine, uri, (name, None), StatusFormat::Html).await
}

#[get("/crate/{name}/latest/status.svg")]
async fn crate_latest_status_svg(
    ThinData(engine): ThinData<Engine>,
    uri: Uri,
    Path((name,)): Path<(String,)>,
) -> actix_web::Result<impl Responder> {
    crate_status(engine, uri, (name, None), StatusFormat::Svg).await
}

#[get("/crate/{name}/latest/shield.json")]
async fn crate_latest_status_shield_json(
    ThinData(engine): ThinData<Engine>,
    uri: Uri,
    Path((name,)): Path<(String,)>,
) -> actix_web::Result<impl Responder> {
    crate_status(engine, uri, (name, None), StatusFormat::ShieldJson).await
}

#[get("/crate/{name}/{version}/status.svg")]
async fn crate_status_svg(
    ThinData(engine): ThinData<Engine>,
    uri: Uri,
    Path((name, version)): Path<(String, String)>,
) -> actix_web::Result<impl Responder> {
    crate_status(engine, uri, (name, Some(version)), StatusFormat::Svg).await
}

#[get("/crate/{name}/{version}/shield.json")]
async fn crate_status_shield_json(
    ThinData(engine): ThinData<Engine>,
    uri: Uri,
    Path((name, version)): Path<(String, String)>,
) -> actix_web::Result<impl Responder> {
    crate_status(engine, uri, (name, Some(version)), StatusFormat::ShieldJson).await
}

async fn crate_status(
    engine: Engine,
    uri: Uri,
    (name, version): (String, Option<String>),
    format: StatusFormat,
) -> actix_web::Result<impl Responder> {
    let is_latest_crate_route = version.is_none();

    let version = match version {
        Some(ver) => ver.to_owned(),
        None => {
            let crate_name = match name.parse() {
                Ok(name) => name,
                Err(_) => return Err(ServerError::BadCratePath.into()),
            };

            match engine
                .find_latest_stable_crate_release(crate_name, VersionReq::STAR)
                .await
            {
                Ok(Some(latest_rel)) => latest_rel.version.to_string(),

                Ok(None) => return Err(ServerError::CrateNotFound.into()),

                Err(err) => {
                    tracing::error!(%err);
                    return Err(ServerError::CrateFetchFailed.into());
                }
            }
        }
    };

    let crate_path_result = CratePath::from_parts(&name, &version);
    let badge_knobs = ExtraConfig::from_query_string(uri.query());

    match crate_path_result {
        Err(err) => {
            tracing::error!(%err);
            Err(ServerError::BadCratePath.into())
        }

        Ok(crate_path) => {
            let badge_tab_mode =
                resolve_badge_tab_mode(&engine, format, &crate_path, is_latest_crate_route).await;

            let analysis_outcome = engine
                .analyze_crate_dependencies(crate_path.clone())
                .await
                .inspect_err(|err| {
                    tracing::error!(%err);
                })
                .ok();

            let response = status_format_analysis(
                analysis_outcome,
                format,
                SubjectPath::Crate(crate_path),
                badge_knobs,
                badge_tab_mode,
            );

            Ok(response)
        }
    }
}

async fn resolve_badge_tab_mode(
    engine: &Engine,
    format: StatusFormat,
    crate_path: &CratePath,
    is_latest_crate_route: bool,
) -> BadgeTabMode {
    if format != StatusFormat::Html {
        return BadgeTabMode::Hidden;
    }

    if is_latest_crate_route {
        return BadgeTabMode::LatestDefault;
    }

    let latest_release = engine
        .find_latest_stable_crate_release(crate_path.name.clone(), VersionReq::STAR)
        .await;

    match latest_release {
        Ok(Some(latest_rel)) => {
            if latest_rel.version == crate_path.version {
                BadgeTabMode::PinnedDefault
            } else {
                BadgeTabMode::Hidden
            }
        }
        Ok(None) => BadgeTabMode::Hidden,
        Err(err) => {
            tracing::error!(%err);
            BadgeTabMode::Hidden
        }
    }
}

fn status_format_analysis(
    analysis_outcome: Option<AnalyzeDependenciesOutcome>,
    format: StatusFormat,
    subject_path: SubjectPath,
    badge_knobs: ExtraConfig,
    badge_tab_mode: BadgeTabMode,
) -> impl Responder {
    match format {
        StatusFormat::Svg => Either::Left(views::badge::response(
            analysis_outcome.as_ref(),
            badge_knobs,
        )),

        StatusFormat::Html => Either::Right(views::html::status::response(
            analysis_outcome,
            subject_path,
            badge_knobs,
            badge_tab_mode,
        )),
        StatusFormat::ShieldJson => Either::Left(views::badge::shield_json_response(
            analysis_outcome.as_ref(),
            badge_knobs,
        )),
    }
}

pub(crate) fn static_files(cfg: &mut ServiceConfig) {
    cfg.service(Resource::new(STATIC_STYLE_CSS_PATH).get(|| async {
        HttpResponse::Ok()
            .insert_header(ContentType(mime::TEXT_CSS_UTF_8))
            .insert_header(ETag(EntityTag::new_strong(
                STATIC_STYLE_CSS_ETAG.to_owned(),
            )))
            .insert_header(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(365000000),
                CacheDirective::Immutable,
            ]))
            .body(assets::STATIC_STYLE_CSS)
    }))
    .service(Resource::new(STATIC_FAVICON_PATH).get(|| async {
        HttpResponse::Ok()
            .insert_header(ContentType(mime::IMAGE_SVG))
            .body(assets::STATIC_FAVICON)
    }))
    .service(Resource::new(STATIC_LINKS_JS_PATH).get(|| async {
        HttpResponse::Ok()
            .insert_header(ContentType(mime::APPLICATION_JAVASCRIPT_UTF_8))
            .insert_header(ETag(EntityTag::new_strong(STATIC_LINKS_JS_ETAG.to_owned())))
            .insert_header(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(365000000),
                CacheDirective::Immutable,
            ]))
            .body(assets::STATIC_LINKS_JS)
    }));
}

pub(crate) async fn not_found() -> impl Responder {
    (
        Html::new(views::html::error::render_404().0),
        StatusCode::NOT_FOUND,
    )
}

static SELF_BASE_URL: LazyLock<String> =
    LazyLock::new(|| env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string()));

/// Configuration options supplied through Get Parameters
#[derive(Debug, Clone, Default)]
pub struct ExtraConfig {
    /// Badge style to show
    style: BadgeStyle,

    /// Whether the inscription _"dependencies"_ should be abbreviated as _"deps"_ in the badge.
    compact: bool,

    /// Custom text on the left (it's the same concept as `label` in shields.io).
    subject: Option<String>,

    /// Path in which the crate resides within the repository
    path: Option<String>,
}

impl ExtraConfig {
    fn from_query_string(qs: Option<&str>) -> Self {
        /// This wrapper can make the deserialization process infallible.
        #[derive(Debug, Clone, Deserialize)]
        #[serde(transparent)]
        struct QueryParam<T>(UntaggedEither<T, String>);

        impl<T> QueryParam<T> {
            fn opt(self) -> Option<T> {
                either::Either::from(self.0).left()
            }
        }

        #[derive(Debug, Clone, Default, Deserialize)]
        struct ExtraConfigPartial {
            style: Option<QueryParam<BadgeStyle>>,
            compact: Option<QueryParam<WrappedBool>>,
            subject: Option<String>,
            path: Option<String>,
        }

        let extra_config = qs
            .and_then(|qs| serde_urlencoded::from_str::<ExtraConfigPartial>(qs).ok())
            .unwrap_or_default();

        Self {
            style: extra_config
                .style
                .and_then(|qp| qp.opt())
                .unwrap_or_default(),
            compact: extra_config
                .compact
                .and_then(|qp| qp.opt())
                .unwrap_or_default()
                .0,
            subject: extra_config
                .subject
                .filter(|t| !t.is_empty())
                .map(|subject| safe_truncate(&subject, MAX_SUBJECT_WIDTH).to_owned()),
            path: extra_config.path,
        }
    }

    /// Returns subject for badge.
    ///
    /// Returns `subject` if set, or "dependencies" / "deps" depending on value of `compact`.
    pub(crate) fn subject(&self) -> &str {
        if let Some(subject) = &self.subject {
            subject
        } else if self.compact {
            "deps"
        } else {
            "dependencies"
        }
    }
}
