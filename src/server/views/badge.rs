use actix_web::{http::header::ContentType, HttpResponse};
use badge::{Badge, BadgeOptions};

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

pub fn response(
    analysis_outcome: Option<&AnalyzeDependenciesOutcome>,
    badge_knobs: ExtraConfig,
) -> HttpResponse {
    let badge = badge(analysis_outcome, badge_knobs).to_svg();

    HttpResponse::Ok()
        .insert_header(ContentType(mime::IMAGE_SVG))
        .body(badge)
}
