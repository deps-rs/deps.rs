use std::collections::BTreeMap;

use hyper::Response;
use hyper::header::ContentType;
use semver::{Version, VersionReq};
use serde_json;

use ::engine::AnalyzeDependenciesOutcome;

#[derive(Debug, Serialize)]
struct AnalyzeDependenciesResponseDetail {
    required: VersionReq,
    latest: Option<Version>,
    outdated: bool
}

#[derive(Debug, Serialize)]
struct AnalyzeDependenciesResponseSingle {
    dependencies: BTreeMap<String, AnalyzeDependenciesResponseDetail>,
    #[serde(rename="dev-dependencies")]
    dev_dependencies: BTreeMap<String, AnalyzeDependenciesResponseDetail>,
    #[serde(rename="build-dependencies")]
    build_dependencies: BTreeMap<String, AnalyzeDependenciesResponseDetail>
}

#[derive(Debug, Serialize)]
struct AnalyzeDependenciesResponse {
    crates: BTreeMap<String, AnalyzeDependenciesResponseSingle>
}

pub fn status_json(analysis_outcome: AnalyzeDependenciesOutcome) -> Response {
    let single = AnalyzeDependenciesResponseSingle {
        dependencies: analysis_outcome.deps.main.into_iter()
            .map(|(name, analyzed)| (name.into(), AnalyzeDependenciesResponseDetail {
                outdated: analyzed.is_outdated(),
                required: analyzed.required,
                latest: analyzed.latest
            })).collect(),
        dev_dependencies: analysis_outcome.deps.dev.into_iter()
            .map(|(name, analyzed)| (name.into(), AnalyzeDependenciesResponseDetail {
                outdated: analyzed.is_outdated(),
                required: analyzed.required,
                latest: analyzed.latest
            })).collect(),
        build_dependencies: analysis_outcome.deps.build.into_iter()
            .map(|(name, analyzed)| (name.into(), AnalyzeDependenciesResponseDetail {
                outdated: analyzed.is_outdated(),
                required: analyzed.required,
                latest: analyzed.latest
            })).collect()
    };

    let multi = AnalyzeDependenciesResponse {
        crates: vec![(analysis_outcome.name.into(), single)].into_iter().collect()
    };

    Response::new()
        .with_header(ContentType::json())
        .with_body(serde_json::to_string(&multi).unwrap())
}
