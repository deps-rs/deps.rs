use std::{
    fmt,
    hash::Hash,
    sync::Arc,
    time::{Duration, Instant},
};

use derive_more::{Display, Error, From};
use hyper::service::Service;
use lru_cache::LruCache;
use slog::{debug, Logger};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Display, From, Error)]
pub struct CacheError<E> {
    inner: E,
}

#[derive(Clone)]
pub struct Cache<S, Req>
where
    S: Service<Req>,
    Req: Hash + Eq,
{
    inner: S,
    duration: Duration,
    cache: Arc<Mutex<LruCache<Req, (Instant, S::Response)>>>,
    logger: Logger,
}

impl<S, Req> fmt::Debug for Cache<S, Req>
where
    S: Service<Req> + fmt::Debug,
    Req: Hash + Eq,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Cache")
            .field("inner", &self.inner)
            .field("duration", &self.duration)
            .finish()
    }
}

impl<S, Req> Cache<S, Req>
where
    S: Service<Req> + fmt::Debug + Clone,
    S::Response: Clone,
    Req: Clone + Eq + Hash + fmt::Debug,
{
    pub fn new(service: S, duration: Duration, capacity: usize, logger: Logger) -> Cache<S, Req> {
        Cache {
            inner: service,
            duration,
            cache: Arc::new(Mutex::new(LruCache::new(capacity))),
            logger,
        }
    }

    pub async fn cached_query(&self, req: Req) -> Result<S::Response, S::Error> {
        let now = Instant::now();

        {
            let mut cache = self.cache.lock().await;

            if let Some((ref valid_until, ref cached_response)) = cache.get_mut(&req) {
                if *valid_until > now {
                    debug!(
                        self.logger, "cache hit";
                        "svc" => format!("{:?}", self.inner),
                        "req" => format!("{:?}", &req)
                    );

                    return Ok(cached_response.clone());
                }
            }
        }

        debug!(
            self.logger, "cache miss";
            "svc" => format!("{:?}", self.inner),
            "req" => format!("{:?}", &req)
        );

        let mut service = self.inner.clone();
        let fresh = service.call(req.clone()).await?;

        {
            let mut cache = self.cache.lock().await;
            cache.insert(req, (now + self.duration, fresh.clone()));
        }

        Ok(fresh)
    }
}
