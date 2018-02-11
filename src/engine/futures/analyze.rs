use failure::Error;
use futures::{Future, Poll, Stream, stream};

use ::models::crates::{AnalyzedDependencies, CrateDeps};

use super::super::Engine;
use super::super::machines::analyzer::DependencyAnalyzer;

const FETCH_RELEASES_CONCURRENCY: usize = 10;

pub struct AnalyzeDependenciesFuture {
    inner: Box<Future<Item=AnalyzedDependencies, Error=Error>>
}

impl AnalyzeDependenciesFuture {
    pub fn new(engine: &Engine, deps: CrateDeps) -> Self {
        let analyzer = DependencyAnalyzer::new(&deps);

        let main_deps = deps.main.into_iter().filter_map(|(name, dep)| {
            if dep.is_external() { Some(name) } else { None }
        });
        let dev_deps = deps.dev.into_iter().filter_map(|(name, dep)| {
            if dep.is_external() { Some(name) } else { None }
        });
        let build_deps = deps.build.into_iter().filter_map(|(name, dep)| {
            if dep.is_external() { Some(name) } else { None }
        });

        let release_futures = engine.fetch_releases(main_deps.chain(dev_deps).chain(build_deps));

        let analyzed_deps_future = stream::iter_ok::<_, Error>(release_futures)
            .buffer_unordered(FETCH_RELEASES_CONCURRENCY)
            .fold(analyzer, |mut analyzer, releases| { analyzer.process(releases); Ok(analyzer) as Result<_, Error> })
            .map(|analyzer| analyzer.finalize());

        AnalyzeDependenciesFuture {
            inner: Box::new(analyzed_deps_future)
        }
    }
}

impl Future for AnalyzeDependenciesFuture {
    type Item = AnalyzedDependencies;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
    }
}
