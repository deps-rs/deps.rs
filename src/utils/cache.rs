use std::{fmt, sync::Arc, time::Duration};

use actix_web::dev::Service;
use lru_time_cache::LruCache;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Cache<S, Req>
where
    S: Service<Req>,
{
    inner: S,
    cache: Arc<Mutex<LruCache<Req, S::Response>>>,
}

impl<S, Req> fmt::Debug for Cache<S, Req>
where
    S: Service<Req> + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Cache")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<S, Req> Cache<S, Req>
where
    S: Service<Req> + fmt::Debug + Clone,
    S::Response: Clone,
    Req: Clone + Eq + Ord + fmt::Debug,
{
    pub fn new(service: S, ttl: Duration, capacity: usize) -> Cache<S, Req> {
        let cache = LruCache::with_expiry_duration_and_capacity(ttl, capacity);

        Cache {
            inner: service,
            cache: Arc::new(Mutex::new(cache)),
        }
    }

    pub async fn cached_query(&self, req: Req) -> Result<S::Response, S::Error> {
        {
            let cache = self.cache.lock().await;

            // `peek` doesn't refresh the entry's TTL, unlike `get`, so
            // frequently requested entries still expire and get refreshed.
            if let Some(cached_response) = cache.peek(&req) {
                tracing::debug!(
                    svc = ?self.inner,
                    req = ?req,
                    cache = "hit",
                );
                return Ok(cached_response.clone());
            }
        }

        tracing::debug!(
            svc = ?self.inner,
            req = ?req,
            cache = "miss",
        );

        let service = self.inner.clone();
        let fresh = service.call(req.clone()).await?;

        {
            let mut cache = self.cache.lock().await;
            cache.insert(req, fresh.clone());
        }

        Ok(fresh)
    }
}
