use actix_web::{HttpResponse, http::header::ContentType};
use badge_maker_rs::{BadgeOptions, Color, make_badge};
use base64::display::Base64Display;
use serde::Serialize;

use crate::{engine::AnalyzeDependenciesOutcome, server::ExtraConfig};

fn badge(
    analysis_outcome: Option<&AnalyzeDependenciesOutcome>,
    badge_knobs: ExtraConfig,
) -> BadgeOptions {
    let subject = badge_knobs.subject().to_owned();

    let (status, color) = match analysis_outcome {
        Some(outcome) => {
            if outcome.any_always_insecure() {
                ("insecure".to_string(), "#e05d44".to_string())
            } else {
                let (outdated, total) = outcome.outdated_ratio();

                if outdated > 0 {
                    (
                        format!("{outdated} of {total} outdated"),
                        "#dfb317".to_string(),
                    )
                } else if total > 0 {
                    if outcome.any_insecure() {
                        ("maybe insecure".to_string(), "#8b1".to_string())
                    } else {
                        ("up to date".to_string(), "#4c1".to_string())
                    }
                } else {
                    ("none".to_string(), "#4c1".to_string())
                }
            }
        }
        None => ("unknown".to_string(), "#9f9f9f".to_string()),
    };

    let mut opts = BadgeOptions::new(status)
        .label(subject)
        .style(badge_knobs.style)
        .build();
    opts.color = Some(Color::literal(color));
    opts
}

fn render_svg(
    analysis_outcome: Option<&AnalyzeDependenciesOutcome>,
    badge_knobs: ExtraConfig,
) -> String {
    let options = badge(analysis_outcome, badge_knobs);

    match make_badge(&options) {
        Ok(svg) => svg,
        Err(err) => {
            tracing::error!(%err, "failed to render badge SVG");
            String::from(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="83" height="20" role="img" aria-label="dependencies: unknown"><title>dependencies: unknown</title><g shape-rendering="crispEdges"><rect width="71" height="20" fill="#555"/><rect x="71" width="12" height="20" fill="#9f9f9f"/></g><g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110"><text x="365" y="140" transform="scale(.1)">dependencies</text><text x="770" y="140" transform="scale(.1)">?</text></g></svg>"##,
            )
        }
    }
}

pub fn svg_data_uri(
    analysis_outcome: Option<&AnalyzeDependenciesOutcome>,
    badge_knobs: ExtraConfig,
) -> String {
    let svg = render_svg(analysis_outcome, badge_knobs);
    format!(
        "data:image/svg+xml;base64,{}",
        Base64Display::new(svg.as_bytes(), &base64::prelude::BASE64_STANDARD)
    )
}

#[derive(Serialize)]
struct ShieldIoJson {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    label: String,
    message: String,
    color: String,
}

pub fn shield_json_response(
    analysis_outcome: Option<&AnalyzeDependenciesOutcome>,
    badge_knobs: ExtraConfig,
) -> HttpResponse {
    let subject = badge_knobs.subject().to_owned();

    let (status, color_hex) = match analysis_outcome {
        Some(outcome) => {
            if outcome.any_always_insecure() {
                ("insecure".to_string(), "#e05d44".to_string())
            } else {
                let (outdated, total) = outcome.outdated_ratio();
                if outdated > 0 {
                    (
                        format!("{outdated} of {total} outdated"),
                        "#dfb317".to_string(),
                    )
                } else if total > 0 {
                    if outcome.any_insecure() {
                        ("maybe insecure".to_string(), "#8b1".to_string())
                    } else {
                        ("up to date".to_string(), "#4c1".to_string())
                    }
                } else {
                    ("none".to_string(), "#4c1".to_string())
                }
            }
        }
        None => ("unknown".to_string(), "#9f9f9f".to_string()),
    };

    let shield_data = ShieldIoJson {
        schema_version: 1,
        label: subject,
        message: status,
        color: color_hex,
    };

    HttpResponse::Ok()
        .content_type(ContentType::json())
        .json(shield_data)
}

pub fn response(
    analysis_outcome: Option<&AnalyzeDependenciesOutcome>,
    badge_knobs: ExtraConfig,
) -> HttpResponse {
    let badge = render_svg(analysis_outcome, badge_knobs);

    HttpResponse::Ok()
        .insert_header(ContentType(mime::IMAGE_SVG))
        .body(badge)
}
