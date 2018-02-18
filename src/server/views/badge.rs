use badge::{Badge, BadgeOptions};
use hyper::Response;
use hyper::header::ContentType;

use ::engine::AnalyzeDependenciesOutcome;

pub fn badge(analysis_outcome: Option<&AnalyzeDependenciesOutcome>) -> Badge {
    let opts = match analysis_outcome {
        Some(outcome) => {
            if outcome.any_insecure() {
                BadgeOptions {
                    subject: "dependencies".into(),
                    status: "insecure".into(),
                    color: "#e05d44".into()
                }
            } else {
                let (outdated, total) = outcome.outdated_ratio();

                if outdated > 0 {
                    BadgeOptions {
                        subject: "dependencies".into(),
                        status: format!("{} of {} outdated", outdated, total),
                        color: "#dfb317".into()
                    }
                } else if total > 0 {
                    BadgeOptions {
                        subject: "dependencies".into(),
                        status: "up to date".into(),
                        color: "#4c1".into()
                    }
                } else {
                    BadgeOptions {
                        subject: "dependencies".into(),
                        status: "none".into(),
                        color: "#4c1".into()
                    }
                }
            }
        },
        None => {
            BadgeOptions {
                subject: "dependencies".into(),
                status: "unknown".into(),
                color: "#9f9f9f".into()
            }
        }
    };

    Badge::new(opts)
        .expect("failed to create badge")
}

pub fn response(analysis_outcome: Option<&AnalyzeDependenciesOutcome>) -> Response {
    Response::new()
        .with_header(ContentType("image/svg+xml;charset=utf-8".parse().unwrap()))
        .with_body(badge(analysis_outcome).to_svg().into_bytes())
}
