use hyper::Response;
use hyper::header::ContentType;

use ::server::assets;
use ::engine::AnalyzeDependenciesOutcome;

pub fn status_svg(analysis_outcome: AnalyzeDependenciesOutcome) -> Response {
    let mut response = Response::new()
        .with_header(ContentType("image/svg+xml;charset=utf-8".parse().unwrap()));
    if analysis_outcome.deps.any_outdated() {
        response.set_body(assets::BADGE_OUTDATED_SVG.to_vec());
    } else {
        response.set_body(assets::BADGE_UPTODATE_SVG.to_vec());
    }
    response
}
