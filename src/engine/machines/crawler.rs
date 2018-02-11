use std::collections::HashMap;

use failure::Error;

use ::parsers::manifest::parse_manifest_toml;
use ::models::crates::{CrateDeps, CrateName, CrateManifest};

pub struct ManifestCrawlerOutput {
    pub crates: Vec<(CrateName, CrateDeps)>
}

pub struct ManifestCrawlerStepOutput {
    pub paths_of_interest: Vec<String>
}

pub struct ManifestCrawler {
    manifests: HashMap<String, CrateManifest>,
    leaf_crates: Vec<(CrateName, CrateDeps)>
}

impl ManifestCrawler {
    pub fn new() -> ManifestCrawler {
        ManifestCrawler {
            manifests: HashMap::new(),
            leaf_crates: vec![]
        }
    }

    pub fn step(&mut self, path: String, raw_manifest: String) -> Result<ManifestCrawlerStepOutput, Error> {
        let manifest = parse_manifest_toml(&raw_manifest)?;
        self.manifests.insert(path, manifest.clone());
        match manifest {
            CrateManifest::Crate(name, deps) => {
                self.leaf_crates.push((name, deps));
            }
        }
        Ok(ManifestCrawlerStepOutput {
            paths_of_interest: vec![]
        })
    }

    pub fn finalize(self) -> ManifestCrawlerOutput {
        ManifestCrawlerOutput {
            crates: self.leaf_crates
        }
    }
}
