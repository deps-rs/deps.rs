use std::sync::Arc;

use crates_index::{Crate, SparseIndex};

use crate::models::crates::CrateName;

#[derive(Clone)]
pub struct ManagedIndex {
    index: Arc<SparseIndex>,
    client: reqwest::Client,
}

impl ManagedIndex {
    pub fn new(client: reqwest::Client) -> Self {
        // the index path is configurable through the `CARGO_HOME` env variable
        let index = Arc::new(SparseIndex::new_cargo_default().unwrap());

        Self { index, client }
    }

    /// Finds crate by name, returning a list of its versions.
    ///
    /// # Errors
    ///
    /// Returns error if HTTP request to crates index fails.
    pub async fn crate_(&self, crate_name: &CrateName) -> anyhow::Result<Option<Crate>> {
        let req = self.index.make_cache_request(crate_name.as_str())?;
        let req = http_to_reqwest_req(req);

        let res = self.client.execute(req).await?;
        let res = reqwest_to_http_res(res).await?;

        Ok(self
            .index
            .parse_cache_response(crate_name.as_str(), res, true)?)
    }
}

/// Converts an `http` request builder from `crates-index` to a `reqwest` request.
fn http_to_reqwest_req(req: http::request::Builder) -> reqwest::Request {
    let req = req
        .body(())
        .expect("request from crates_index crate should be valid");

    let req = http::Request::from_parts(req.into_parts().0, Vec::new());

    reqwest::Request::try_from(req).expect("request from crates_index crate should be valid")
}

/// Converts an `http` request builder from `crates-index` to a `reqwest` request.
///
/// # Errors
///
/// Returns error if reading HTTP response fails (e.g.: connection is lost while streaming payload).
async fn reqwest_to_http_res(res: reqwest::Response) -> anyhow::Result<http::Response<Vec<u8>>> {
    use http_body_util::BodyExt as _;

    let (res, body) = http::Response::from(res).into_parts();

    let body = body.collect().await?.to_bytes().to_vec();

    Ok(http::Response::from_parts(res, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_impl_all!(ManagedIndex: Send, Sync);
}
