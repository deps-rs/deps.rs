use std::sync::Arc;

use rustsec::{
    cargo_lock,
    database::{self, Database},
};
use semver::Version;

use crate::models::crates::{
    AnalyzedDependencies, AnalyzedDependency, CrateDeps, CrateName, CrateRelease,
};

pub struct DependencyAnalyzer {
    deps: AnalyzedDependencies,
    advisory_db: Option<Arc<Database>>,
}

impl DependencyAnalyzer {
    pub fn new(deps: &CrateDeps, advisory_db: Option<Arc<Database>>) -> DependencyAnalyzer {
        DependencyAnalyzer {
            deps: AnalyzedDependencies::new(deps),
            advisory_db,
        }
    }

    fn process_single(
        name: &CrateName,
        dep: &mut AnalyzedDependency,
        ver: &Version,
        advisory_db: Option<&Database>,
    ) {
        if dep.required.matches(ver) {
            if let Some(ref mut current_latest_that_matches) = dep.latest_that_matches {
                if *current_latest_that_matches < *ver {
                    *current_latest_that_matches = ver.clone();
                }
            } else {
                dep.latest_that_matches = Some(ver.clone());
            }

            let name: cargo_lock::Name = name.as_ref().parse().unwrap();
            let version: cargo_lock::Version = ver.to_string().parse().unwrap();
            let query = database::Query::crate_scope()
                .package_name(name)
                .package_version(version);

            if let Some(db) = advisory_db {
                let vulnerabilities: Vec<_> =
                    db.query(&query).into_iter().map(|v| v.to_owned()).collect();
                if vulnerabilities.is_empty() {
                    dep.has_safe_matching_release = true;
                } else {
                    dep.vulnerabilities = vulnerabilities;
                }
            }
        }
        if ver.pre.is_empty() {
            if let Some(ref mut current_latest) = dep.latest {
                if *current_latest < *ver {
                    *current_latest = ver.clone();
                }
            } else {
                dep.latest = Some(ver.clone());
            }
        }
    }

    pub fn process<I: IntoIterator<Item = CrateRelease>>(&mut self, releases: I) {
        let advisory_db = self.advisory_db.as_deref();

        for release in releases.into_iter().filter(|r| !r.yanked) {
            if let Some(main_dep) = self.deps.main.get_mut(&release.name) {
                DependencyAnalyzer::process_single(
                    &release.name,
                    main_dep,
                    &release.version,
                    advisory_db,
                )
            }
            if let Some(dev_dep) = self.deps.dev.get_mut(&release.name) {
                DependencyAnalyzer::process_single(
                    &release.name,
                    dev_dep,
                    &release.version,
                    advisory_db,
                )
            }
            if let Some(build_dep) = self.deps.build.get_mut(&release.name) {
                DependencyAnalyzer::process_single(
                    &release.name,
                    build_dep,
                    &release.version,
                    advisory_db,
                )
            }
        }
    }

    pub fn finalize(self) -> AnalyzedDependencies {
        self.deps
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::models::crates::{CrateDep, CrateDeps, CrateRelease};

    fn analyze_security(required: &str, releases: &[(&str, bool)]) -> AnalyzedDependencies {
        let mut deps = CrateDeps::default();
        deps.main.insert(
            "example".parse().unwrap(),
            CrateDep::External(required.parse().unwrap()),
        );

        let db = Database::open(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/test-advisories"),
        )
        .unwrap();
        let mut analyzer = DependencyAnalyzer::new(&deps, Some(Arc::new(db)));

        for &(version, yanked) in releases {
            analyzer.process([CrateRelease {
                name: "example".parse().unwrap(),
                version: version.parse().unwrap(),
                deps: CrateDeps::default(),
                yanked,
            }]);
        }

        analyzer.finalize()
    }

    #[test]
    fn safe_release_in_range_prevents_insecure_status_in_any_order() {
        for releases in [
            [("1.0.0", false), ("1.1.0", false), ("1.2.0", false)],
            [("1.2.0", false), ("1.1.0", false), ("1.0.0", false)],
        ] {
            let analyzed = analyze_security("1", &releases);

            assert_eq!(analyzed.count_insecure(), 0);
        }
    }

    #[test]
    fn safe_release_outside_range_does_not_prevent_insecure_status() {
        let analyzed = analyze_security("1", &[("1.0.0", false), ("2.0.0", false)]);

        assert_eq!(analyzed.count_insecure(), 1);
    }

    #[test]
    fn yanked_safe_release_does_not_prevent_insecure_status() {
        let analyzed = analyze_security("1", &[("1.0.0", false), ("1.1.0", true)]);

        assert_eq!(analyzed.count_insecure(), 1);
    }

    #[test]
    fn no_matching_releases_are_not_insecure() {
        let analyzed = analyze_security("3", &[("1.0.0", false)]);

        assert_eq!(analyzed.count_insecure(), 0);
    }

    #[test]
    fn tracks_latest_without_matching() {
        let mut deps = CrateDeps::default();
        deps.main.insert(
            "hyper".parse().unwrap(),
            CrateDep::External("^0.11.0".parse().unwrap()),
        );

        let mut analyzer = DependencyAnalyzer::new(&deps, None);
        analyzer.process(vec![
            CrateRelease {
                name: "hyper".parse().unwrap(),
                version: "0.10.0".parse().unwrap(),
                deps: Default::default(),
                yanked: false,
            },
            CrateRelease {
                name: "hyper".parse().unwrap(),
                version: "0.10.1".parse().unwrap(),
                deps: Default::default(),
                yanked: false,
            },
        ]);

        let analyzed = analyzer.finalize();

        assert_eq!(
            analyzed.main.get("hyper").unwrap().latest_that_matches,
            None
        );
        assert_eq!(
            analyzed.main.get("hyper").unwrap().latest,
            Some("0.10.1".parse().unwrap())
        );
    }

    #[test]
    fn tracks_latest_that_matches() {
        let mut deps = CrateDeps::default();
        deps.main.insert(
            "hyper".parse().unwrap(),
            CrateDep::External("^0.10.0".parse().unwrap()),
        );

        let mut analyzer = DependencyAnalyzer::new(&deps, None);
        analyzer.process(vec![
            CrateRelease {
                name: "hyper".parse().unwrap(),
                version: "0.10.0".parse().unwrap(),
                deps: Default::default(),
                yanked: false,
            },
            CrateRelease {
                name: "hyper".parse().unwrap(),
                version: "0.10.1".parse().unwrap(),
                deps: Default::default(),
                yanked: false,
            },
            CrateRelease {
                name: "hyper".parse().unwrap(),
                version: "0.11.0".parse().unwrap(),
                deps: Default::default(),
                yanked: false,
            },
        ]);

        let analyzed = analyzer.finalize();

        assert_eq!(
            analyzed.main.get("hyper").unwrap().latest_that_matches,
            Some("0.10.1".parse().unwrap())
        );
        assert_eq!(
            analyzed.main.get("hyper").unwrap().latest,
            Some("0.11.0".parse().unwrap())
        );
    }

    #[test]
    fn skips_yanked_releases() {
        let mut deps = CrateDeps::default();
        deps.main.insert(
            "hyper".parse().unwrap(),
            CrateDep::External("^0.10.0".parse().unwrap()),
        );

        let mut analyzer = DependencyAnalyzer::new(&deps, None);
        analyzer.process(vec![
            CrateRelease {
                name: "hyper".parse().unwrap(),
                version: "0.10.0".parse().unwrap(),
                deps: Default::default(),
                yanked: false,
            },
            CrateRelease {
                name: "hyper".parse().unwrap(),
                version: "0.10.1".parse().unwrap(),
                deps: Default::default(),
                yanked: true,
            },
        ]);

        let analyzed = analyzer.finalize();

        assert_eq!(
            analyzed.main.get("hyper").unwrap().latest_that_matches,
            Some("0.10.0".parse().unwrap())
        );
        assert_eq!(
            analyzed.main.get("hyper").unwrap().latest,
            Some("0.10.0".parse().unwrap())
        );
    }

    #[test]
    fn skips_prereleases() {
        let mut deps = CrateDeps::default();
        deps.main.insert(
            "hyper".parse().unwrap(),
            CrateDep::External("^0.10.0".parse().unwrap()),
        );

        let mut analyzer = DependencyAnalyzer::new(&deps, None);
        analyzer.process(vec![
            CrateRelease {
                name: "hyper".parse().unwrap(),
                version: "0.10.0".parse().unwrap(),
                deps: Default::default(),
                yanked: false,
            },
            CrateRelease {
                name: "hyper".parse().unwrap(),
                version: "0.10.1-alpha".parse().unwrap(),
                deps: Default::default(),
                yanked: false,
            },
        ]);

        let analyzed = analyzer.finalize();

        assert_eq!(
            analyzed.main.get("hyper").unwrap().latest_that_matches,
            Some("0.10.0".parse().unwrap())
        );
        assert_eq!(
            analyzed.main.get("hyper").unwrap().latest,
            Some("0.10.0".parse().unwrap())
        );
    }
}
