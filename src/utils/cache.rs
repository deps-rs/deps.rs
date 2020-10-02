use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    hash::Hash,
    sync::Mutex,
    time::{Duration, Instant},
};

use derive_more::{Display, Error, From};
use hyper::service::Service;
use lru_cache::LruCache;
use slog::{debug, Logger};

#[derive(Debug, Clone, Display, From, Error)]
pub struct CacheError<E> {
    inner: E,
}

pub struct Cache<S, Req>
where
    S: Service<Req>,
    Req: Hash + Eq,
{
    inner: S,
    duration: Duration,
    cache: Mutex<LruCache<Req, (Instant, S::Response)>>,
    logger: Logger,
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
    S::Response: Clone,
    Req: Clone + Eq + Hash,
{
    pub fn new(service: S, duration: Duration, capacity: usize, logger: Logger) -> Cache<S, Req> {
        Cache {
            inner: service,
            duration,
            cache: Mutex::new(LruCache::new(capacity)),
            logger,
        }
    }

    pub async fn cached_query(&mut self, req: Req) -> Result<S::Response, S::Error> {
        let now = Instant::now();

        {
            let mut cache = self.cache.lock().expect("cache lock poisoned");

            if let Some((ref valid_until, ref cached_response)) = cache.get_mut(&req) {
                if *valid_until > now {
                    debug!(self.logger, "cache hit");
                    return Ok(cached_response.clone());
                }
            }
        }

        debug!(self.logger, "cache miss");

        let fresh = self.inner.call(req.clone()).await?;

        {
            let mut cache = self.cache.lock().expect("cache lock poisoned");
            cache.insert(req, (now + self.duration, fresh.clone()));
        }

        Ok(fresh)
    }
}
