use std::str;
use std::sync::Arc;

use anyhow::{anyhow, ensure, Error};
use futures::{future, future::done, Future, IntoFuture, Stream};
use hyper::{Error as HyperError, Method, Request, Response};
use rustsec::database::Database;
use rustsec::repository::DEFAULT_URL;
use tokio_service::Service;

#[derive(Debug, Clone)]
pub struct FetchAdvisoryDatabase<S>(pub S);

impl<S> Service for FetchAdvisoryDatabase<S>
where
    S: Service<Request = Request, Response = Response, Error = HyperError> + Clone + 'static,
    S::Future: 'static,
{
    type Request = ();
    type Response = Arc<Database>;
    type Error = Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, _req: ()) -> Self::Future {
        let service = self.0.clone();

        Box::new(done(
            rustsec::Database::fetch()
                .map(|db| Arc::new(db))
                .map_err(|err| anyhow!("err fetching rustsec DB")),
        ))
    }
}

// #[derive(Debug, Clone)]
// pub struct FetchAdvisoryDatabase<S>(pub S);

// impl<S> Service for FetchAdvisoryDatabase<S>
// where
//     S: Service<Request = Request, Response = Response, Error = HyperError> + Clone + 'static,
//     S::Future: 'static,
// {
//     type Request = ();
//     type Response = Arc<Database>;
//     type Error = Error;
//     type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

//     fn call(&self, _req: ()) -> Self::Future {
//         let service = self.0.clone();

//         let uri_future = DEFAULT_URL.parse().into_future().from_err();

//         Box::new(uri_future.and_then(move |uri| {
//             let request = Request::new(Method::Get, uri);

//             service.call(request).from_err().and_then(|response| {
//                 let status = response.status();
//                 if !status.is_success() {
//                     future::Either::A(future::err(anyhow!(
//                         "Status code {} when fetching advisory db",
//                         status
//                     )))
//                 } else {
//                     let body_future = response.body().concat2().from_err();
//                     let decode_future = body_future.and_then(|body| {
//                         Ok(Arc::new(Database::from_toml(str::from_utf8(
//                             &body,
//                         )?)?))
//                     });
//                     future::Either::B(decode_future)
//                 }
//             })
//         }))
//     }
// }
