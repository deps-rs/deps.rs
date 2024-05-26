use actix_http::{body::MessageBody, header::CONTENT_TYPE, Response, StatusCode};
use badge::{Badge, BadgeOptions};

use crate::{engine::AnalyzeDependenciesOutcome, server::ExtraConfig};

pub fn badge(
    analysis_outcome: Option<&AnalyzeDependenciesOutcome>,
    badge_knobs: ExtraConfig,
) -> Badge {
    let subject = if badge_knobs.compact {
        "deps"
    } else {
        "dependencies"
    }
    .to_owned();

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

pub fn response(
    analysis_outcome: Option<&AnalyzeDependenciesOutcome>,
    badge_knobs: ExtraConfig,
) -> Response<impl MessageBody> {
    let badge = badge(analysis_outcome, badge_knobs).to_svg();

    Response::build(StatusCode::OK)
        .insert_header((CONTENT_TYPE, "image/svg+xml; charset=utf-8"))
        .body(badge)
}
