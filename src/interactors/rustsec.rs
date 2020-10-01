use std::{sync::Arc, task::Context, task::Poll};

use anyhow::Error;
use futures::{future::ready, future::BoxFuture};
use hyper::{service::Service, Body, Error as HyperError, Request, Response};
use rustsec::database::Database;

#[derive(Debug, Clone)]
pub struct FetchAdvisoryDatabase<S>(pub S);

impl<S> Service<()> for FetchAdvisoryDatabase<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = HyperError> + Clone,
    S::Future: 'static,
{
    type Response = Arc<Database>;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: should be this when async client is used again
        // self.0.poll_ready(cx).map_err(|err| err.into())
        Poll::Ready(Ok(()))
    }

    // TODO: make fetch async again
    fn call(&mut self, _req: ()) -> Self::Future {
        let _service = self.0.clone();

        Box::pin(ready(
            rustsec::Database::fetch().map(Arc::new).map_err(Into::into),
        ))
    }
}

// #[derive(Debug, Clone)]
// pub struct FetchAdvisoryDatabase<S>(pub S);

// impl<S> Service for FetchAdvisoryDatabase<S>
// where
//     S: Service<Request = Request, Response = Response, Error = HyperError> + Clone,
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
