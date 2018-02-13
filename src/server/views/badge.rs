use badge::{Badge, BadgeOptions};
use hyper::Response;
use hyper::header::ContentType;

use ::engine::AnalyzeDependenciesOutcome;

pub fn svg(analysis_outcome: Option<&AnalyzeDependenciesOutcome>) -> Vec<u8> {
    let opts = match analysis_outcome {
        Some(outcome) => {
            if outcome.any_outdated() {
                BadgeOptions {
                    subject: "dependencies".into(),
                    status: "outdated".into(),
                    color: "#dfb317".into()
                }
            } else {
                BadgeOptions {
                    subject: "dependencies".into(),
                    status: "up to date".into(),
                    color: "#4c1".into()
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
        .to_svg()
        .into_bytes()
}

pub fn response(analysis_outcome: Option<&AnalyzeDependenciesOutcome>) -> Response {
    Response::new()
        .with_header(ContentType("image/svg+xml;charset=utf-8".parse().unwrap()))
        .with_body(svg(analysis_outcome))
}
