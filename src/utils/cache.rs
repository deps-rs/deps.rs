use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::time::{Duration, Instant};
use std::ops::Deref;
use std::sync::Mutex;

use failure::Error;
use futures::{Future, Poll};
use futures::future::{FromErr, Shared, SharedItem};
use lru_cache::LruCache;
use shared_failure::SharedFailure;
use tokio_service::Service;

pub struct Cache<S>
    where S: Service<Error=Error>,
          S::Request: Hash + Eq
{
    inner: S,
    duration: Duration,
    cache: Mutex<LruCache<S::Request, (Instant, Shared<FromErr<S::Future, SharedFailure>>)>>
}

impl<S> Debug for Cache<S>
    where S: Service<Error=Error> + Debug,
          S::Request: Hash + Eq
{
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        fmt.debug_struct("Cache")
            .field("inner", &self.inner)
            .field("duration", &self.duration)
            .finish()
    }
}

impl<S> Cache<S> 
    where S: Service<Error=Error>,
          S::Request: Hash + Eq
{
    pub fn new(service: S, duration: Duration, capacity: usize) -> Cache<S> {
        Cache {
            inner: service,
            duration: duration,
            cache: Mutex::new(LruCache::new(capacity))
        }
    }
}

impl<S> Service for Cache<S>
    where S: Service<Error=Error>,
          S::Request: Clone + Hash + Eq
{
    type Request = S::Request;
    type Response = CachedItem<S::Response>;
    type Error = SharedFailure;
    type Future = Cached<S::Future>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let now = Instant::now();
        let mut cache = self.cache.lock().expect("lock poisoned");
        if let Some(&mut (valid_until, ref shared_future)) = cache.get_mut(&req) {
            if valid_until > now {
                if let Some(Ok(_)) = shared_future.peek() {
                    return Cached(shared_future.clone());
                }
            }
        }
        let shared_future = self.inner.call(req.clone()).from_err().shared();
        cache.insert(req, (now + self.duration, shared_future.clone()));
        Cached(shared_future)
    }
}

pub struct Cached<F: Future<Error=Error>>(Shared<FromErr<F, SharedFailure>>);

impl<F> Debug for Cached<F>
    where F: Future<Error=Error> + Debug,
          F::Item: Debug
{
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        self.0.fmt(fmt)
    }
}

impl<F: Future<Error=Error>> Future for Cached<F> {
    type Item = CachedItem<F::Item>;
    type Error = SharedFailure;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
            .map_err(|err| (*err).clone())
            .map(|async| async.map(CachedItem))
    }
}

#[derive(Debug)]
pub struct CachedItem<T>(SharedItem<T>);

impl<T> Deref for CachedItem<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0.deref()
    }
}
