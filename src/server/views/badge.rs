use actix_web::{HttpResponse, http::header::ContentType};
use badge::{Badge, BadgeOptions};
use serde::Serialize;

use crate::{engine::AnalyzeDependenciesOutcome, server::ExtraConfig};

pub fn badge(
    analysis_outcome: Option<&AnalyzeDependenciesOutcome>,
    badge_knobs: ExtraConfig,
) -> Badge {
    let subject = badge_knobs.subject().to_owned();

    let opts = match analysis_outcome {
        Some(outcome) => {
            if outcome.any_always_insecure() {
                BadgeOptions {
                    subject,
                    status: "insecure".into(),
                    color: "#e05d44".into(),
                    style: badge_knobs.style,
                }
            } else {
                let (outdated, total) = outcome.outdated_ratio();

                if outdated > 0 {
                    BadgeOptions {
                        subject,
                        status: format!("{outdated} of {total} outdated"),
                        color: "#dfb317".into(),
                        style: badge_knobs.style,
                    }
                } else if total > 0 {
                    if outcome.any_insecure() {
                        BadgeOptions {
                            subject,
                            status: "maybe insecure".into(),
                            color: "#8b1".into(),
                            style: badge_knobs.style,
                        }
                    } else {
                        BadgeOptions {
                            subject,
                            status: "up to date".into(),
                            color: "#4c1".into(),
                            style: badge_knobs.style,
                        }
                    }
                } else {
                    BadgeOptions {
                        subject,
                        status: "none".into(),
                        color: "#4c1".into(),
                        style: badge_knobs.style,
                    }
                }
            }
        }
        None => BadgeOptions {
            subject,
            status: "unknown".into(),
            color: "#9f9f9f".into(),
            style: badge_knobs.style,
        },
    };

    Badge::new(opts)
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
    let badge = badge(analysis_outcome, badge_knobs).to_svg();

    HttpResponse::Ok()
        .insert_header(ContentType(mime::IMAGE_SVG))
        .body(badge)
}
