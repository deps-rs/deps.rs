use hyper::Response;
use hyper::header::ContentType;

use ::server::assets;
use ::engine::AnalyzeDependenciesOutcome;

pub fn status_svg(analysis_outcome: Option<AnalyzeDependenciesOutcome>) -> Response {
    let mut response = Response::new()
        .with_header(ContentType("image/svg+xml;charset=utf-8".parse().unwrap()));
    if let Some(outcome) = analysis_outcome {
        if outcome.deps.any_outdated() {
            response.set_body(assets::BADGE_OUTDATED_SVG.to_vec());
        } else {
            response.set_body(assets::BADGE_UPTODATE_SVG.to_vec());
        }
    } else {
        response.set_body(assets::BADGE_UNKNOWN_SVG.to_vec());
    }
    response
}
