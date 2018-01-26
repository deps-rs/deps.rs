use std::collections::BTreeMap;
use std::collections::btree_map::{Entry as BTreeMapEntry};

use semver::{Version, VersionReq};

use ::models::crates::{CrateName, CrateDeps, CrateRelease, AnalyzedDependency, AnalyzedDependencies};

pub struct DependencyAnalyzer {
    deps: AnalyzedDependencies
}

impl DependencyAnalyzer {
    pub fn new(deps: &CrateDeps) -> DependencyAnalyzer {
        DependencyAnalyzer {
            deps: AnalyzedDependencies::new(deps)
        }
    }

    fn process_single(dep: &mut AnalyzedDependency, ver: &Version) {
        if dep.required.matches(&ver) {
            if let Some(ref mut current_latest_that_matches) = dep.latest_that_matches {
                if *current_latest_that_matches < *ver {
                    *current_latest_that_matches = ver.clone();
                }
            } else {
                dep.latest_that_matches = Some(ver.clone());
            }
        }
        if let Some(ref mut current_latest) = dep.latest {
            if *current_latest < *ver {
                *current_latest = ver.clone();
            }
        } else {
            dep.latest = Some(ver.clone());
        }
    }

    pub fn process<I: IntoIterator<Item=CrateRelease>>(&mut self, releases: I) {
        for release in releases.into_iter().filter(|r| !r.yanked) {
            if let Some(main_dep) = self.deps.main.get_mut(&release.name) {
                DependencyAnalyzer::process_single(main_dep, &release.version)
            }
            if let Some(dev_dep) = self.deps.dev.get_mut(&release.name) {
                DependencyAnalyzer::process_single(dev_dep, &release.version)
            }
            if let Some(build_dep) = self.deps.build.get_mut(&release.name) {
                DependencyAnalyzer::process_single(build_dep, &release.version)
            }
        }
    }

    pub fn finalize(self) -> AnalyzedDependencies {
        self.deps
    }
}
