use anyhow::{Error, anyhow};
use indexmap::IndexMap;
use relative_path::RelativePathBuf;
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::models::crates::{CrateDep, CrateDeps, CrateManifest, CrateName};

#[derive(Serialize, Deserialize, Debug)]
struct CargoTomlComplexDependency {
    git: Option<String>,
    path: Option<RelativePathBuf>,
    version: Option<String>,
    package: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum CargoTomlDependency {
    Simple(String),
    Complex(CargoTomlComplexDependency),
}

#[derive(Serialize, Deserialize, Debug)]
struct CargoTomlPackage {
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct CargoTomlWorkspace {
    #[serde(default)]
    members: Vec<RelativePathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CargoTomlTargetDependencies {
    #[serde(default)]
    dependencies: IndexMap<String, CargoTomlDependency>,
    #[serde(rename = "dev-dependencies")]
    #[serde(default)]
    dev_dependencies: IndexMap<String, CargoTomlDependency>,
    #[serde(rename = "build-dependencies")]
    #[serde(default)]
    build_dependencies: IndexMap<String, CargoTomlDependency>,
}

#[derive(Serialize, Deserialize, Debug)]
struct CargoToml {
    #[serde(default)]
    package: Option<CargoTomlPackage>,
    #[serde(default)]
    workspace: Option<CargoTomlWorkspace>,
    #[serde(default)]
    dependencies: IndexMap<String, CargoTomlDependency>,
    #[serde(rename = "dev-dependencies")]
    #[serde(default)]
    dev_dependencies: IndexMap<String, CargoTomlDependency>,
    #[serde(rename = "build-dependencies")]
    #[serde(default)]
    build_dependencies: IndexMap<String, CargoTomlDependency>,
    #[serde(default)]
    target: IndexMap<String, CargoTomlTargetDependencies>,
}

fn extract_target_dependencies_into(
    target: IndexMap<String, CargoTomlTargetDependencies>,
    deps: &mut IndexMap<String, CargoTomlDependency>,
    dev_deps: &mut IndexMap<String, CargoTomlDependency>,
    build_deps: &mut IndexMap<String, CargoTomlDependency>,
) {
    for target_deps in target.into_values() {
        deps.extend(target_deps.dependencies);
        dev_deps.extend(target_deps.dev_dependencies);
        build_deps.extend(target_deps.build_dependencies);
    }
}

fn convert_dependency(
    cargo_dep: (String, CargoTomlDependency),
) -> Option<Result<(CrateName, CrateDep), Error>> {
    match cargo_dep {
        (name, CargoTomlDependency::Simple(string)) => {
            Some(name.parse::<CrateName>().and_then(|parsed_name| {
                string
                    .parse::<VersionReq>()
                    .map_err(|err| err.into())
                    .map(|version| (parsed_name, CrateDep::External(version)))
            }))
        }
        (name, CargoTomlDependency::Complex(cplx)) => {
            if cplx.git.is_some() {
                None
            } else if cplx.path.is_some() {
                cplx.path.map(|path| {
                    name.parse::<CrateName>()
                        .map(|parsed_name| (parsed_name, CrateDep::Internal(path)))
                })
            } else {
                cplx.version.as_deref().map(|version| {
                    let name = cplx.package.as_deref().unwrap_or(&name);
                    name.parse::<CrateName>().and_then(|parsed_name| {
                        version
                            .parse::<VersionReq>()
                            .map_err(|err| err.into())
                            .map(|version| (parsed_name, CrateDep::External(version)))
                    })
                })
            }
        }
    }
}

pub fn parse_manifest_toml(input: &str) -> Result<CrateManifest, Error> {
    let mut cargo_toml = toml::de::from_str::<CargoToml>(input)?;

    let mut package_part = None;
    let mut workspace_part = None;

    if let Some(package) = cargo_toml.package {
        let crate_name = package.name.parse::<CrateName>()?;

        extract_target_dependencies_into(
            cargo_toml.target,
            &mut cargo_toml.dependencies,
            &mut cargo_toml.dev_dependencies,
            &mut cargo_toml.build_dependencies,
        );

        let dependencies = cargo_toml
            .dependencies
            .into_iter()
            .filter_map(convert_dependency)
            .collect::<Result<IndexMap<_, _>, _>>()?;
        let dev_dependencies = cargo_toml
            .dev_dependencies
            .into_iter()
            .filter_map(convert_dependency)
            .collect::<Result<IndexMap<_, _>, _>>()?;
        let build_dependencies = cargo_toml
            .build_dependencies
            .into_iter()
            .filter_map(convert_dependency)
            .collect::<Result<IndexMap<_, _>, _>>()?;

        let deps = CrateDeps {
            main: dependencies,
            dev: dev_dependencies,
            build: build_dependencies,
        };

        package_part = Some((crate_name, deps));
    }

    if let Some(workspace) = cargo_toml.workspace {
        workspace_part = Some(workspace.members);
    }

    match (package_part, workspace_part) {
        (Some((name, deps)), None) => Ok(CrateManifest::Package(name, deps)),
        (None, Some(members)) => Ok(CrateManifest::Workspace { members }),
        (Some((name, deps)), Some(members)) => Ok(CrateManifest::Mixed {
            name,
            deps,
            members,
        }),
        (None, None) => Err(anyhow!("neither workspace nor package found in manifest")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::crates::CrateManifest;

    #[test]
    fn parse_workspace_without_members_declaration() {
        let toml = r#"[package]
name = "symbolic"

[workspace]

[dependencies]
symbolic-common = { version = "2.0.6", path = "common" }
"#;

        let manifest = parse_manifest_toml(toml).unwrap();

        match manifest {
            CrateManifest::Mixed {
                name,
                deps,
                members,
            } => {
                assert_eq!(name.as_ref(), "symbolic");
                assert_eq!(deps.main.len(), 1);
                assert_eq!(deps.dev.len(), 0);
                assert_eq!(deps.build.len(), 0);
                assert_eq!(members.len(), 0);
            }
            _ => panic!("expected mixed manifest"),
        }
    }

    #[test]
    fn parse_manifest_with_renamed_deps() {
        let toml = r#"[package]
name = "symbolic"

[dependencies]
symbolic-common_crate = { version = "2.0.6", package = "symbolic-common" }
"#;

        let manifest = parse_manifest_toml(toml).unwrap();

        match manifest {
            CrateManifest::Package(name, deps) => {
                assert_eq!(name.as_ref(), "symbolic");
                assert_eq!(deps.main.len(), 1);
                assert_eq!(deps.dev.len(), 0);
                assert_eq!(deps.build.len(), 0);

                let name: CrateName = "symbolic-common".parse().unwrap();
                assert!(deps.main.get(&name).is_some());
            }
            _ => panic!("expected package manifest"),
        }
    }
}

#[test]
fn parse_manifest_with_target_dependencies() {
    let toml = r#"[package]
name = "platform-specific"

[dependencies]
serde = "1.0"

[target.'cfg(unix)'.dependencies]
nix = { version = "0.28", features = ["sched"] }

[target.'cfg(windows)'.dev-dependencies]
winapi = "0.3"

[target.'cfg(any(target_os = "android", target_os = "dragonfly", target_os = "freebsd", target_os = "linux"))'.build-dependencies]
cc = "1.0"
"#;

    let manifest = parse_manifest_toml(toml).unwrap();

    match manifest {
        CrateManifest::Package(name, deps) => {
            assert_eq!(name.as_ref(), "platform-specific");

            assert_eq!(deps.main.len(), 2);
            let serde_name: CrateName = "serde".parse().unwrap();
            assert!(deps.main.get(&serde_name).is_some());
            let nix_name: CrateName = "nix".parse().unwrap();
            assert!(deps.main.get(&nix_name).is_some());

            assert_eq!(deps.dev.len(), 1);
            let winapi_name: CrateName = "winapi".parse().unwrap();
            assert!(deps.dev.get(&winapi_name).is_some());

            assert_eq!(deps.build.len(), 1);
            let cc_name: CrateName = "cc".parse().unwrap();
            assert!(deps.build.get(&cc_name).is_some());
        }
        _ => panic!("expected package manifest"),
    }
}
