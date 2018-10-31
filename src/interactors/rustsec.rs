use std::sync::Arc;

use failure::Error;
use futures::{done, Future};
use futures::future::poll_fn;
use rustsec::db::AdvisoryDatabase;
use tokio_threadpool::blocking;
use tokio_service::Service;

#[derive(Debug, Clone)]
pub struct FetchAdvisoryDatabase;

impl Service for FetchAdvisoryDatabase
{
    type Request = ();
    type Response = Arc<AdvisoryDatabase>;
    type Error = Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>+ Send>;

    fn call(&self, _req: ()) -> Self::Future {
        Box::new(
            poll_fn(move || blocking(|| AdvisoryDatabase::fetch()))
                .map_err(Error::from)
                .and_then(|x| done(x).map_err(Error::from))
                .map(Arc::new),
        )
    }
}
