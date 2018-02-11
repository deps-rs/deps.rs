use std::collections::HashMap;
use std::path::PathBuf;

use failure::Error;
use ordermap::map::OrderMap;

use ::parsers::manifest::parse_manifest_toml;
use ::models::crates::{CrateDeps, CrateName, CrateManifest};

pub struct ManifestCrawlerOutput {
    pub crates: OrderMap<CrateName, CrateDeps>
}

pub struct ManifestCrawlerStepOutput {
    pub paths_of_interest: Vec<PathBuf>
}

pub struct ManifestCrawler {
    manifests: HashMap<PathBuf, CrateManifest>,
    leaf_crates: OrderMap<CrateName, CrateDeps>
}

impl ManifestCrawler {
    pub fn new() -> ManifestCrawler {
        ManifestCrawler {
            manifests: HashMap::new(),
            leaf_crates: OrderMap::new()
        }
    }

    pub fn step(&mut self, path: PathBuf, raw_manifest: String) -> Result<ManifestCrawlerStepOutput, Error> {
        let manifest = parse_manifest_toml(&raw_manifest)?;
        self.manifests.insert(path.clone(), manifest.clone());

        let mut output = ManifestCrawlerStepOutput {
            paths_of_interest: vec![]
        };

        match manifest {
            CrateManifest::Package(name, deps) => {
                self.leaf_crates.insert(name, deps);
            },
            CrateManifest::Workspace { members } => {
                for mut member in members {
                    if !member.ends_with("*") {
                        output.paths_of_interest.push(path.clone().join(member));
                    }
                }
            },
            CrateManifest::Mixed { name, deps, members } => {
                self.leaf_crates.insert(name, deps);
                for mut member in members {
                    if !member.ends_with("*") {
                        output.paths_of_interest.push(path.clone().join(member));
                    }
                }
            }
        }

        Ok(output)
    }

    pub fn finalize(self) -> ManifestCrawlerOutput {
        ManifestCrawlerOutput {
            crates: self.leaf_crates
        }
    }
}

#[cfg(test)]
mod tests {
    use semver::VersionReq;

    use super::ManifestCrawler;

    #[test]
    fn simple_package_manifest() {
        let manifest = r#"
[package]
name = "simpleton"
"#;
        let mut crawler = ManifestCrawler::new();
        let step_output = crawler.step("Cargo.toml".into(), manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 0);
        let output = crawler.finalize();
        assert_eq!(output.crates.len(), 1);
        assert_eq!(output.crates["simpleton"].main.len(), 0);
        assert_eq!(output.crates["simpleton"].dev.len(), 0);
        assert_eq!(output.crates["simpleton"].build.len(), 0);
    }

    #[test]
    fn more_complex_package_manifest() {
        let manifest = r#"
[package]
name = "more-complex"
[dependencies]
foo = "0.30.0"
bar = { version = "1.2.0", optional = true }
[dev-dependencies]
quickcheck = "0.5"
[build-dependencies]
codegen = "0.0.1"
"#;
        let mut crawler = ManifestCrawler::new();
        let step_output = crawler.step("/Cargo.toml".into(), manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 0);
        let output = crawler.finalize();
        assert_eq!(output.crates.len(), 1);
        assert_eq!(output.crates["more-complex"].main.len(), 2);
        assert_eq!(output.crates["more-complex"].main.get("foo").unwrap(),
            &VersionReq::parse("0.30.0").unwrap());
        assert_eq!(output.crates["more-complex"].main.get("bar").unwrap(),
            &VersionReq::parse("1.2.0").unwrap());
        assert_eq!(output.crates["more-complex"].dev.len(), 1);
        assert_eq!(output.crates["more-complex"].dev.get("quickcheck").unwrap(),
            &VersionReq::parse("0.5").unwrap());
        assert_eq!(output.crates["more-complex"].build.len(), 1);
        assert_eq!(output.crates["more-complex"].build.get("codegen").unwrap(),
            &VersionReq::parse("0.0.1").unwrap());
    }

    #[test]
    fn simple_workspace_manifest() {
        let manifest = r#"
[workspace]
members = [
  "lib/",
  "codegen/",
  "contrib/",
]
"#;
        let mut crawler = ManifestCrawler::new();
        let step_output = crawler.step("/".into(), manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 3);
        assert_eq!(step_output.paths_of_interest[0].to_str().unwrap(), "/lib/");
        assert_eq!(step_output.paths_of_interest[1].to_str().unwrap(), "/codegen/");
        assert_eq!(step_output.paths_of_interest[2].to_str().unwrap(), "/contrib/");
    }

    #[test]
    fn glob_workspace_manifest() {
        let manifest = r#"
[workspace]
members = [
  "lib/",
  "tests/*",
]
"#;
        let mut crawler = ManifestCrawler::new();
        let step_output = crawler.step("/".into(), manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 1);
        assert_eq!(step_output.paths_of_interest[0].to_str().unwrap(), "/lib/");
    }

    #[test]
    fn mixed_package_and_workspace_manifest() {
        let futures_manifest = r#"
[package]
name = "futures"

[dependencies]

[workspace]
members = ["futures-cpupool"]
"#;

        let futures_cpupool_manifest = r#"
[package]
name = "futures-cpupool"

[dependencies]
num_cpus = "1.0"

[dependencies.futures]
path = ".."
version = "0.1"
default-features = false
features = ["use_std"]
"#;

        let mut crawler = ManifestCrawler::new();
        let step_output = crawler.step("/".into(), futures_manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 1);
        assert_eq!(step_output.paths_of_interest[0].to_str().unwrap(), "/futures-cpupool");
        let step_output = crawler.step("/futures-cpupool".into(), futures_cpupool_manifest.to_string()).unwrap();
        assert_eq!(step_output.paths_of_interest.len(), 0);
        let output = crawler.finalize();
        assert_eq!(output.crates.len(), 2);
        assert_eq!(output.crates["futures"].main.len(), 0);
        assert_eq!(output.crates["futures"].dev.len(), 0);
        assert_eq!(output.crates["futures"].build.len(), 0);
        assert_eq!(output.crates["futures-cpupool"].main.len(), 1);
        assert_eq!(output.crates["futures-cpupool"].main.get("num_cpus").unwrap(),
            &VersionReq::parse("1.0").unwrap());
        assert_eq!(output.crates["futures-cpupool"].dev.len(), 0);
        assert_eq!(output.crates["futures-cpupool"].build.len(), 0);
    }
}
