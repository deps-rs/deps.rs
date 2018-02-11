use semver::Version;

use ::models::crates::{CrateDeps, CrateRelease, AnalyzedDependency, AnalyzedDependencies};

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
        if !ver.is_prerelease() {
            if let Some(ref mut current_latest) = dep.latest {
                if *current_latest < *ver {
                    *current_latest = ver.clone();
                }
            } else {
                dep.latest = Some(ver.clone());
            }
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

#[cfg(test)]
mod tests {
    use models::crates::{CrateDep, CrateDeps, CrateRelease};
    use super::DependencyAnalyzer;

    #[test]
    fn tracks_latest_without_matching() {
        let mut deps = CrateDeps::default();
        deps.main.insert("hyper".parse().unwrap(), CrateDep::External("^0.11.0".parse().unwrap()));

        let mut analyzer = DependencyAnalyzer::new(&deps);
        analyzer.process(vec![
            CrateRelease { name: "hyper".parse().unwrap(), version: "0.10.0".parse().unwrap(), yanked: false },
            CrateRelease { name: "hyper".parse().unwrap(), version: "0.10.1".parse().unwrap(), yanked: false }
        ]);

        let analyzed = analyzer.finalize();

        assert_eq!(analyzed.main.get("hyper").unwrap().latest_that_matches, None);
        assert_eq!(analyzed.main.get("hyper").unwrap().latest, Some("0.10.1".parse().unwrap()));
    }

    #[test]
    fn tracks_latest_that_matches() {
        let mut deps = CrateDeps::default();
        deps.main.insert("hyper".parse().unwrap(), CrateDep::External("^0.10.0".parse().unwrap()));

        let mut analyzer = DependencyAnalyzer::new(&deps);
        analyzer.process(vec![
            CrateRelease { name: "hyper".parse().unwrap(), version: "0.10.0".parse().unwrap(), yanked: false },
            CrateRelease { name: "hyper".parse().unwrap(), version: "0.10.1".parse().unwrap(), yanked: false },
            CrateRelease { name: "hyper".parse().unwrap(), version: "0.11.0".parse().unwrap(), yanked: false }
        ]);

        let analyzed = analyzer.finalize();

        assert_eq!(analyzed.main.get("hyper").unwrap().latest_that_matches, Some("0.10.1".parse().unwrap()));
        assert_eq!(analyzed.main.get("hyper").unwrap().latest, Some("0.11.0".parse().unwrap()));
    }

    #[test]
    fn skips_yanked_releases() {
        let mut deps = CrateDeps::default();
        deps.main.insert("hyper".parse().unwrap(), CrateDep::External("^0.10.0".parse().unwrap()));

        let mut analyzer = DependencyAnalyzer::new(&deps);
        analyzer.process(vec![
            CrateRelease { name: "hyper".parse().unwrap(), version: "0.10.0".parse().unwrap(), yanked: false },
            CrateRelease { name: "hyper".parse().unwrap(), version: "0.10.1".parse().unwrap(), yanked: true },
        ]);

        let analyzed = analyzer.finalize();

        assert_eq!(analyzed.main.get("hyper").unwrap().latest_that_matches, Some("0.10.0".parse().unwrap()));
        assert_eq!(analyzed.main.get("hyper").unwrap().latest, Some("0.10.0".parse().unwrap()));
    }

    #[test]
    fn skips_prereleases() {
        let mut deps = CrateDeps::default();
        deps.main.insert("hyper".parse().unwrap(), CrateDep::External("^0.10.0".parse().unwrap()));

        let mut analyzer = DependencyAnalyzer::new(&deps);
        analyzer.process(vec![
            CrateRelease { name: "hyper".parse().unwrap(), version: "0.10.0".parse().unwrap(), yanked: false },
            CrateRelease { name: "hyper".parse().unwrap(), version: "0.10.1-alpha".parse().unwrap(), yanked: false },
        ]);

        let analyzed = analyzer.finalize();

        assert_eq!(analyzed.main.get("hyper").unwrap().latest_that_matches, Some("0.10.0".parse().unwrap()));
        assert_eq!(analyzed.main.get("hyper").unwrap().latest, Some("0.10.0".parse().unwrap()));
    }
}
