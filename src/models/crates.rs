use std::{borrow::Borrow, str::FromStr};

use anyhow::{anyhow, Error};
use indexmap::IndexMap;
use relative_path::RelativePathBuf;
use rustsec::Advisory;
use semver::{Version, VersionReq};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CratePath {
    pub name: CrateName,
    pub version: Version,
}

impl CratePath {
    pub fn from_parts(name: &str, version: &str) -> Result<CratePath, Error> {
        Ok(CratePath {
            name: name.parse()?,
            version: version.parse()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrateName(String);

impl From<CrateName> for String {
    fn from(crate_name: CrateName) -> String {
        crate_name.0
    }
}

impl Borrow<str> for CrateName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CrateName {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl FromStr for CrateName {
    type Err = Error;

    fn from_str(input: &str) -> Result<CrateName, Error> {
        let is_valid = input
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');

        if !is_valid {
            Err(anyhow!("failed to validate crate name: {}", input))
        } else {
            Ok(CrateName(input.to_string()))
        }
    }
}

#[derive(Clone, Debug)]
pub struct CrateRelease {
    pub name: CrateName,
    pub version: Version,
    pub deps: CrateDeps,
    pub yanked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrateDep {
    External(VersionReq),
    Internal(RelativePathBuf),
}

impl CrateDep {
    pub fn is_external(&self) -> bool {
        matches!(self, CrateDep::External(_))
    }
}

#[derive(Clone, Debug, Default)]
pub struct CrateDeps {
    pub main: IndexMap<CrateName, CrateDep>,
    pub dev: IndexMap<CrateName, CrateDep>,
    pub build: IndexMap<CrateName, CrateDep>,
}

#[derive(Debug)]
pub struct AnalyzedDependency {
    pub required: VersionReq,
    pub latest_that_matches: Option<Version>,
    pub latest: Option<Version>,
    pub vulnerabilities: Vec<Advisory>,
}

impl AnalyzedDependency {
    pub fn new(required: VersionReq) -> AnalyzedDependency {
        AnalyzedDependency {
            required,
            latest_that_matches: None,
            latest: None,
            vulnerabilities: Vec::new(),
        }
    }

    /// Check whether this dependency has at least one known vulnerability
    /// in any version in the required version range.
    ///
    /// Note that the vulnerability may (or not) already be patched
    /// in the latest version(s) in the range.
    pub fn is_insecure(&self) -> bool {
        !self.vulnerabilities.is_empty()
    }

    /// Check whether this dependency has at laest one known vulnerability
    /// even when the latest version in the required range is used.
    pub fn is_always_insecure(&self) -> bool {
        if let Some(latest) = &self.latest {
            self.vulnerabilities
                .iter()
                .any(|a| a.versions.is_vulnerable(latest))
        } else {
            self.is_insecure()
        }
    }

    pub fn is_outdated(&self) -> bool {
        self.latest > self.latest_that_matches
    }

    pub fn deps_rs_path(&self, name: &str) -> String {
        match &self.latest_that_matches {
            Some(version) => ["/crate/", name, "/", version.to_string().as_str()].concat(),
            None => ["/crate/", name].concat(),
        }
    }
}

#[derive(Debug)]
pub struct AnalyzedDependencies {
    pub main: IndexMap<CrateName, AnalyzedDependency>,
    pub dev: IndexMap<CrateName, AnalyzedDependency>,
    pub build: IndexMap<CrateName, AnalyzedDependency>,
}

impl AnalyzedDependencies {
    pub fn new(deps: &CrateDeps) -> AnalyzedDependencies {
        let main = deps
            .main
            .iter()
            .filter_map(|(name, dep)| {
                if let CrateDep::External(ref req) = dep {
                    Some((name.clone(), AnalyzedDependency::new(req.clone())))
                } else {
                    None
                }
            })
            .collect();
        let dev = deps
            .dev
            .iter()
            .filter_map(|(name, dep)| {
                if let CrateDep::External(ref req) = dep {
                    Some((name.clone(), AnalyzedDependency::new(req.clone())))
                } else {
                    None
                }
            })
            .collect();
        let build = deps
            .build
            .iter()
            .filter_map(|(name, dep)| {
                if let CrateDep::External(ref req) = dep {
                    Some((name.clone(), AnalyzedDependency::new(req.clone())))
                } else {
                    None
                }
            })
            .collect();
        AnalyzedDependencies { main, dev, build }
    }

    /// Counts the total number of main and build dependencies
    pub fn count_total(&self) -> usize {
        self.main.len() + self.build.len()
    }

    /// Returns the number of outdated main and build dependencies
    pub fn count_outdated(&self) -> usize {
        let main_outdated = self
            .main
            .iter()
            .filter(|&(_, dep)| dep.is_outdated())
            .count();
        let build_outdated = self
            .build
            .iter()
            .filter(|&(_, dep)| dep.is_outdated())
            .count();
        main_outdated + build_outdated
    }

    /// Returns the number of insecure main and build dependencies
    pub fn count_insecure(&self) -> usize {
        let main_insecure = self
            .main
            .iter()
            .filter(|&(_, dep)| dep.is_insecure())
            .count();
        let build_insecure = self
            .build
            .iter()
            .filter(|&(_, dep)| dep.is_insecure())
            .count();
        main_insecure + build_insecure
    }

    /// Returns the number of main and build dependencies
    /// which are vulnerable to security issues,
    /// even they are updated to the latest version in the required range.
    pub fn count_always_insecure(&self) -> usize {
        let main_insecure = self
            .main
            .iter()
            .filter(|&(_, dep)| dep.is_always_insecure())
            .count();
        let build_insecure = self
            .build
            .iter()
            .filter(|&(_, dep)| dep.is_always_insecure())
            .count();
        main_insecure + build_insecure
    }

    /// Checks if any outdated main or build dependencies exist
    pub fn any_outdated(&self) -> bool {
        let main_any_outdated = self.main.iter().any(|(_, dep)| dep.is_outdated());
        let build_any_outdated = self.build.iter().any(|(_, dep)| dep.is_outdated());
        main_any_outdated || build_any_outdated
    }

    /// Counts the number of outdated `dev-dependencies`
    pub fn count_dev_outdated(&self) -> usize {
        self.dev
            .iter()
            .filter(|&(_, dep)| dep.is_outdated())
            .count()
    }

    /// Counts the number of insecure `dev-dependencies`
    pub fn count_dev_insecure(&self) -> usize {
        self.dev
            .iter()
            .filter(|&(_, dep)| dep.is_insecure())
            .count()
    }

    /// Returns `true` if any dev-dependencies are either insecure or outdated.
    pub fn any_dev_issues(&self) -> bool {
        self.dev
            .iter()
            .any(|(_, dep)| dep.is_outdated() || dep.is_insecure())
    }
}

#[derive(Clone, Debug)]
pub enum CrateManifest {
    Package(CrateName, CrateDeps),
    Workspace {
        members: Vec<RelativePathBuf>,
    },
    Mixed {
        name: CrateName,
        deps: CrateDeps,
        members: Vec<RelativePathBuf>,
    },
}
