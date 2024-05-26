use std::{fmt, sync::Arc, time::Duration};

use derive_more::{Display, Error, From};
use hyper::service::Service;
use lru_time_cache::LruCache;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Display, From, Error)]
pub struct CacheError<E> {
    inner: E,
}

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
            let mut cache = self.cache.lock().await;

            if let Some(cached_response) = cache.get(&req) {
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

        let mut service = self.inner.clone();
        let fresh = service.call(req.clone()).await?;

        {
            let mut cache = self.cache.lock().await;
            cache.insert(req, fresh.clone());
        }

        Ok(fresh)
    }
}
