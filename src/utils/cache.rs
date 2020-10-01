use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    hash::Hash,
    sync::Mutex,
    task::Context,
    task::Poll,
    time::{Duration, Instant},
};

use anyhow::Error;
use hyper::service::Service;
use lru_cache::LruCache;

pub struct Cache<S, Req>
where
    S: Service<Req>,
    Req: Hash + Eq,
{
    inner: S,
    duration: Duration,
    #[allow(unused)]
    cache: Mutex<LruCache<Req, (Instant, S::Response)>>,
}

impl<S, Req> Debug for Cache<S, Req>
where
    S: Service<Req> + Debug,
    Req: Hash + Eq,
{
    fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
        fmt.debug_struct("Cache")
            .field("inner", &self.inner)
            .field("duration", &self.duration)
            .finish()
    }
}

impl<S, Req> Cache<S, Req>
where
    S: Service<Req>,
    Req: Hash + Eq,
{
    pub fn new(service: S, duration: Duration, capacity: usize) -> Cache<S, Req> {
        Cache {
            inner: service,
            duration,
            cache: Mutex::new(LruCache::new(capacity)),
        }
    }
}

impl<S, Req> Service<Req> for Cache<S, Req>
where
    S: Service<Req, Error = Error>,
    S::Response: Clone,
    Req: Clone + Hash + Eq,
{
    type Response = S::Response;
    type Error = Error;
    // WAS: type Future = Cached<S::Future>;
    // type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
    type Future = S::Future;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Req) -> Self::Future {
        // TODO: re-add caching
        // Box::pin({
        // let now = Instant::now();
        // let mut cache = self.cache.lock().expect("lock poisoned");

        // if let Some(&mut (valid_until, ref cached_response)) = cache.get_mut(&req) {
        //     if valid_until > now {
        //         return Box::pin(ok(cached_response.clone()));
        //     }
        // }

        self.inner.call(req)
        // .and_then(|response| {
        //     // cache.insert(req, (now + self.duration, response.clone()));
        //     ok(response)
        // })
        // })
    }
}

// pub struct Cached<F: Future>(Shared<F>);

// impl<F> Debug for Cached<F>
// where
//     F: Future + Debug,
//     F::Output: Debug,
// {
//     fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
//         self.0.fmt(fmt)
//     }
// }

// // WAS: impl<F: Future<Error = Error>> Future for Cached<F> {
// impl<F: Future> Future for Cached<F> {
//     type Output = Result<F::Output, Error>;

//     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         self.0
//             .poll()
//             .map_err(|_err| anyhow!("TODO: shared error not clone-able"))
//     }
// }
