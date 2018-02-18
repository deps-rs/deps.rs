use std::str;
use std::sync::Arc;

use failure::Error;
use futures::{Future, IntoFuture, Stream, future};
use hyper::{Error as HyperError, Method, Request, Response};
use rustsec::ADVISORY_DB_URL;
use rustsec::db::AdvisoryDatabase;
use tokio_service::Service;

#[derive(Debug, Clone)]
pub struct FetchAdvisoryDatabase<S>(pub S);

impl<S> Service for FetchAdvisoryDatabase<S>
    where S: Service<Request=Request, Response=Response, Error=HyperError> + Clone + 'static,
          S::Future: 'static
{
    type Request = ();
    type Response = Arc<AdvisoryDatabase>;
    type Error = Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, _req: ()) -> Self::Future {
        let service = self.0.clone();

        let uri_future = ADVISORY_DB_URL.parse().into_future().from_err();

        Box::new(uri_future.and_then(move |uri| {
            let request = Request::new(Method::Get, uri);

            service.call(request).from_err().and_then(|response| {
                let status = response.status();
                if !status.is_success() {
                    future::Either::A(future::err(format_err!("Status code {} when fetching advisory db", status)))
                } else {
                    let body_future = response.body().concat2().from_err();
                    let decode_future = body_future
                        .and_then(|body| Ok(Arc::new(AdvisoryDatabase::from_toml(str::from_utf8(&body)?)?)));
                    future::Either::B(decode_future)
                }
            })
        }))
    }
}
