use std::collections::BTreeMap;
use std::str::FromStr;

use semver::{Version, VersionReq};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CrateName(String);

impl Into<String> for CrateName {
    fn into(self) -> String {
        self.0
    }
}

#[derive(Debug)]
pub struct CrateNameValidationError;

impl AsRef<str> for CrateName {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl FromStr for CrateName {
    type Err = CrateNameValidationError;

    fn from_str(input: &str) -> Result<CrateName, CrateNameValidationError> {
        let is_valid = input.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-'
        });

        if !is_valid {
            Err(CrateNameValidationError)
        } else {
            Ok(CrateName(input.to_string()))
        }
    }
}

#[derive(Debug)]
pub struct CrateRelease {
    pub name: CrateName,
    pub version: Version,
    pub yanked: bool
}

#[derive(Debug)]
pub struct CrateDeps {
    pub main: BTreeMap<CrateName, VersionReq>,
    pub dev: BTreeMap<CrateName, VersionReq>,
    pub build: BTreeMap<CrateName, VersionReq>
}

#[derive(Debug)]
pub struct AnalyzedDependency {
    pub required: VersionReq,
    pub latest_that_matches: Option<Version>,
    pub latest: Option<Version>
}

impl AnalyzedDependency {
    pub fn new(required: VersionReq) -> AnalyzedDependency {
        AnalyzedDependency {
            required,
            latest_that_matches: None,
            latest: None
        }
    }

    pub fn is_outdated(&self) -> bool {
        self.latest > self.latest_that_matches
    }
}

#[derive(Debug)]
pub struct AnalyzedDependencies {
    pub main: BTreeMap<CrateName, AnalyzedDependency>,
    pub dev: BTreeMap<CrateName, AnalyzedDependency>,
    pub build: BTreeMap<CrateName, AnalyzedDependency>
}

impl AnalyzedDependencies {
    pub fn new(deps: &CrateDeps) -> AnalyzedDependencies {
        let main = deps.main.iter().map(|(name, req)| {
            (name.clone(), AnalyzedDependency::new(req.clone()))
        }).collect();
        let dev = deps.dev.iter().map(|(name, req)| {
            (name.clone(), AnalyzedDependency::new(req.clone()))
        }).collect();
        let build = deps.build.iter().map(|(name, req)| {
            (name.clone(), AnalyzedDependency::new(req.clone()))
        }).collect();
        AnalyzedDependencies { main, dev, build }
    }
}

#[derive(Debug)]
pub enum CrateManifest {
    Crate(CrateName, CrateDeps)
}
