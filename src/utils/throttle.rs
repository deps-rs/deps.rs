use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::time::{Duration, Instant};
use std::ops::Deref;
use std::sync::Mutex;

use failure::{Error, Fail};
use futures::{Future, Poll};
use futures::future::{Shared, SharedError, SharedItem};
use tokio_service::Service;

pub struct Throttle<S>
    where S: Service<Request=(), Error=Error>
{
    inner: S,
    duration: Duration,
    current: Mutex<Option<(Instant, Shared<S::Future>)>>
}

impl<S> Debug for Throttle<S>
    where S: Service<Request=(), Error=Error> + Debug
{
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        fmt.debug_struct("Throttle")
            .field("inner", &self.inner)
            .field("duration", &self.duration)
            .finish()
    }
}

impl<S> Throttle<S> 
    where S: Service<Request=(), Error=Error>
{
    pub fn new(service: S, duration: Duration) -> Throttle<S> {
        Throttle {
            inner: service,
            duration,
            current: Mutex::new(None)
        }
    }
}

impl<S> Service for Throttle<S>
    where S: Service<Request=(), Error=Error>
{
    type Request = ();
    type Response = ThrottledItem<S::Response>;
    type Error = ThrottledError;
    type Future = Throttled<S::Future>;

    fn call(&self, _: ()) -> Self::Future {
        let now = Instant::now();
        let mut current = self.current.lock().expect("lock poisoned");
        if let Some((valid_until, ref shared_future)) = *current {
            if valid_until > now {
                if let Some(Ok(_)) = shared_future.peek() {
                    return Throttled(shared_future.clone());
                }
            }
        }
        let shared_future = self.inner.call(()).shared();
        *current = Some((now + self.duration, shared_future.clone()));
        Throttled(shared_future)
    }
}

pub struct Throttled<F: Future>(Shared<F>);

impl<F> Debug for Throttled<F>
    where F: Future + Debug,
          F::Item: Debug,
          F::Error: Debug
{
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        self.0.fmt(fmt)
    }
}

impl<F: Future<Error=Error>> Future for Throttled<F> {
    type Item = ThrottledItem<F::Item>;
    type Error = ThrottledError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
            .map_err(ThrottledError)
            .map(|async| async.map(ThrottledItem))
    }
}

#[derive(Debug)]
pub struct ThrottledItem<T>(SharedItem<T>);

impl<T> Deref for ThrottledItem<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0.deref()
    }
}

#[derive(Debug)]
pub struct ThrottledError(SharedError<Error>);

impl Fail for ThrottledError {
    fn cause(&self) -> Option<&Fail> {
        Some(self.0.cause())
    }

    fn backtrace(&self) -> Option<&::failure::Backtrace> {
        Some(self.0.backtrace())
    }

    fn causes(&self) -> ::failure::Causes {
        self.0.causes()
    }
}

impl Display for ThrottledError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        Display::fmt(&self.0, f)
    }
}
