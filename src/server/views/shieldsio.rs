use std::borrow::Cow;

use hyper::header::CONTENT_TYPE;
use hyper::{Body, Response};
use serde_json::json;

use crate::engine::AnalyzeDependenciesOutcome;

struct EndpointData<'a> {
    message: Cow<'a, str>,
    color: &'a str,
}

fn data(analysis_outcome: Option<&AnalyzeDependenciesOutcome>) -> EndpointData<'_> {
    match analysis_outcome {
        Some(outcome) => {
            if outcome.any_always_insecure() {
                EndpointData {
                    message: "insecure".into(),
                    color: "#e05d44",
                }
            } else {
                let (outdated, total) = outcome.outdated_ratio();

                if outdated > 0 {
                    EndpointData {
                        message: format!("{} of {} outdated", outdated, total).into(),
                        color: "#dfb317",
                    }
                } else if total > 0 {
                    if outcome.any_insecure() {
                        EndpointData {
                            message: "maybe insecure".into(),
                            color: "#88bb11",
                        }
                    } else {
                        EndpointData {
                            message: "up to date".into(),
                            color: "#44cc11",
                        }
                    }
                } else {
                    EndpointData {
                        message: "none".into(),
                        color: "#44cc11",
                    }
                }
            }
        }
        None => EndpointData {
            message: "unknown".into(),
            color: "#9f9f9f",
        },
    }
}

pub fn response(analysis_outcome: Option<&AnalyzeDependenciesOutcome>) -> Response<Body> {
    let data = data(analysis_outcome);
    let json = json! {{
        "schemaVersion": 1,
        "label": "dependencies",
        "message": data.message,
        "color": data.color
    }};

    Response::builder()
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .body(Body::from(json.to_string()))
        .unwrap()
}
