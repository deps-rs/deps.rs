use actix_web::{
    HttpResponse, ResponseError,
    http::{StatusCode, header::ContentType},
};
use derive_more::Display;
use maud::Markup;

use crate::server::views::html::error::{render, render_404};

#[derive(Debug, Display)]
pub(crate) enum ServerError {
    #[display("Could not retrieve popular items")]
    PopularItemsFailed,

    #[display("Crate not found")]
    CrateNotFound,

    #[display("Could not parse crate path")]
    BadCratePath,

    #[display("Could not fetch crate information")]
    CrateFetchFailed,

    #[display("Could not parse repository path")]
    BadRepoPath,

    #[display("Crate/repo analysis failed")]
    AnalysisFailed(Markup),
}

impl ResponseError for ServerError {
    fn status_code(&self) -> StatusCode {
        match self {
            ServerError::PopularItemsFailed => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::CrateNotFound => StatusCode::NOT_FOUND,
            ServerError::BadCratePath => StatusCode::BAD_REQUEST,
            ServerError::CrateFetchFailed => StatusCode::NOT_FOUND,
            ServerError::BadRepoPath => StatusCode::BAD_REQUEST,
            ServerError::AnalysisFailed(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let mut res = HttpResponse::build(self.status_code());
        let res = res.insert_header(ContentType::html());

        match self {
            ServerError::PopularItemsFailed => res.body(render(self.to_string(), "").0),

            ServerError::CrateNotFound => res.body(render_404().0),

            ServerError::BadCratePath => res.body(
                render(
                    self.to_string(),
                    "Please make sure to provide a valid crate name and version.",
                )
                .0,
            ),

            ServerError::CrateFetchFailed => res.body(
                render(
                    self.to_string(),
                    "Please make sure to provide a valid crate name.",
                )
                .0,
            ),

            ServerError::BadRepoPath => res.body(
                render(
                    self.to_string(),
                    "Please make sure to provide a valid repository path.",
                )
                .0,
            ),

            Self::AnalysisFailed(html) => res.body(html.0.clone()),
        }
    }
}
