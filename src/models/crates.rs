use std::borrow::Borrow;
use std::str::FromStr;

use failure::Error;
use indexmap::IndexMap;
use relative_path::RelativePathBuf;
use semver::{Version, VersionReq};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CratePath {
    pub name: CrateName,
    pub version: Version
}

impl CratePath {
    pub fn from_parts(name: &str, version: &str) -> Result<CratePath, Error> {
        Ok(CratePath {
            name: name.parse()?,
            version: version.parse()?
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrateName(String);

impl Into<String> for CrateName {
    fn into(self) -> String {
        self.0
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
        let is_valid = input.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-'
        });

        if !is_valid {
            Err(format_err!("failed to validate crate name: {}", input))
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
    pub yanked: bool
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrateDep {
    External(VersionReq),
    Internal(RelativePathBuf)
}

impl CrateDep {
    pub fn is_external(&self) -> bool {
        if let &CrateDep::External(_) = self {
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CrateDeps {
    pub main: IndexMap<CrateName, CrateDep>,
    pub dev: IndexMap<CrateName, CrateDep>,
    pub build: IndexMap<CrateName, CrateDep>
}

#[derive(Debug)]
pub struct AnalyzedDependency {
    pub required: VersionReq,
    pub latest_that_matches: Option<Version>,
    pub latest: Option<Version>,
    pub insecure: bool
}

impl AnalyzedDependency {
    pub fn new(required: VersionReq) -> AnalyzedDependency {
        AnalyzedDependency {
            required,
            latest_that_matches: None,
            latest: None,
            insecure: false
        }
    }

    pub fn is_outdated(&self) -> bool {
        self.latest > self.latest_that_matches
    }
}

#[derive(Debug)]
pub struct AnalyzedDependencies {
    pub main: IndexMap<CrateName, AnalyzedDependency>,
    pub dev: IndexMap<CrateName, AnalyzedDependency>,
    pub build: IndexMap<CrateName, AnalyzedDependency>
}

impl AnalyzedDependencies {
    pub fn new(deps: &CrateDeps) -> AnalyzedDependencies {
        let main = deps.main.iter().filter_map(|(name, dep)| {
            if let &CrateDep::External(ref req) = dep {
                Some((name.clone(), AnalyzedDependency::new(req.clone())))
            } else {
                None
            }
        }).collect();
        let dev = deps.dev.iter().filter_map(|(name, dep)| {
            if let &CrateDep::External(ref req) = dep {
                Some((name.clone(), AnalyzedDependency::new(req.clone())))
            } else {
                None
            }
        }).collect();
        let build = deps.build.iter().filter_map(|(name, dep)| {
            if let &CrateDep::External(ref req) = dep {
                Some((name.clone(), AnalyzedDependency::new(req.clone())))
            } else {
                None
            }
        }).collect();
        AnalyzedDependencies { main, dev, build }
    }

    pub fn count_total(&self) -> usize {
        self.main.len() + self.dev.len() + self.build.len()
    }

    pub fn count_outdated(&self) -> usize {
        let main_outdated = self.main.iter()
            .filter(|&(_, dep)| dep.is_outdated())
            .count();
        let dev_outdated = self.dev.iter()
            .filter(|&(_, dep)| dep.is_outdated())
            .count();
        let build_outdated = self.build.iter()
            .filter(|&(_, dep)| dep.is_outdated())
            .count();
        main_outdated + dev_outdated + build_outdated
    }

     pub fn count_insecure(&self) -> usize {
        let main_insecure = self.main.iter()
            .filter(|&(_, dep)| dep.insecure)
            .count();
        let dev_insecure = self.dev.iter()
            .filter(|&(_, dep)| dep.insecure)
            .count();
        let build_insecure = self.build.iter()
            .filter(|&(_, dep)| dep.insecure)
            .count();
        main_insecure + dev_insecure + build_insecure
    } 

    pub fn any_outdated(&self) -> bool {
        let main_any_outdated = self.main.iter()
            .any(|(_, dep)| dep.is_outdated());
        let dev_any_outdated = self.dev.iter()
            .any(|(_, dep)| dep.is_outdated());
        let build_any_outdated = self.build.iter()
            .any(|(_, dep)| dep.is_outdated());
        main_any_outdated || dev_any_outdated || build_any_outdated
    }
}

#[derive(Clone, Debug)]
pub enum CrateManifest {
    Package(CrateName, CrateDeps),
    Workspace { members: Vec<RelativePathBuf> },
    Mixed { name: CrateName, deps: CrateDeps, members: Vec<RelativePathBuf> }
}
