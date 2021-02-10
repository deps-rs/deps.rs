use font_awesome_as_a_crate::{svg as fa, Type as FaType};
use hyper::{Body, Response};
use indexmap::IndexMap;
use maud::{html, Markup, PreEscaped};
use pulldown_cmark::{html, Parser};
use rustsec::advisory::Advisory;
use semver::Version;

use crate::engine::AnalyzeDependenciesOutcome;
use crate::models::crates::{AnalyzedDependencies, AnalyzedDependency, CrateName};
use crate::models::repo::RepoSite;
use crate::models::SubjectPath;
use crate::server::views::badge;

fn get_crates_url(name: impl AsRef<str>) -> String {
    format!("https://crates.io/crates/{}", name.as_ref())
}

fn get_crates_version_url(name: impl AsRef<str>, version: &Version) -> String {
    format!("https://crates.io/crates/{}/{}", name.as_ref(), version)
}

fn dependency_tables(crate_name: &CrateName, deps: &AnalyzedDependencies) -> Markup {
    html! {
        h2 class="title is-3" {
            "Crate "
            code { (crate_name.as_ref()) }
        }

        @if deps.main.is_empty() && deps.dev.is_empty() && deps.build.is_empty() {
            p class="notification has-text-centered" { "No external dependencies! 🙌" }
        }

        @if !deps.main.is_empty() {
            (dependency_table("Dependencies", &deps.main))
        }

        @if !deps.dev.is_empty() {
            (dependency_table("Dev dependencies", &deps.dev))
        }

        @if !deps.build.is_empty() {
            (dependency_table("Build dependencies", &deps.build))
        }
    }
}

fn dependency_table(title: &str, deps: &IndexMap<CrateName, AnalyzedDependency>) -> Markup {
    let count_total = deps.len();
    let count_insecure = deps.iter().filter(|&(_, dep)| dep.is_insecure()).count();
    let count_outdated = deps.iter().filter(|&(_, dep)| dep.is_outdated()).count();

    let fa_cube = PreEscaped(fa(FaType::Solid, "cube").unwrap());

    html! {
        h3 class="title is-4" { (title) }
        p class="subtitle is-5" {
            (match (count_outdated, count_insecure) {
                (0, 0) => format!("({} total, all up-to-date)", count_total),
                (0, _) => format!("({} total, {} insecure)", count_total, count_insecure),
                (_, 0) => format!("({} total, {} outdated)", count_total, count_outdated),
                (_, _) => format!("({} total, {} outdated, {} insecure)", count_total, count_outdated, count_insecure),
            })
        }

        table class="table is-fullwidth is-striped is-hoverable" {
            thead {
                tr {
                    th { "Crate" }
                    th class="has-text-right" { "Required" }
                    th class="has-text-right" { "Latest" }
                    th class="has-text-right" { "Status" }
                }
            }
            tbody {
                @for (name, dep) in deps {
                    tr {
                        td {
                            a class="has-text-grey" href=(get_crates_url(&name)) {
                                { (fa_cube) }
                            }
                            { "\u{00A0}" } // non-breaking space
                            a href=(dep.deps_rs_path(name.as_ref())) { (name.as_ref()) }
                        }
                        td class="has-text-right" { code { (dep.required.to_string()) } }
                        td class="has-text-right" {
                            @if let Some(ref latest) = dep.latest {
                                code { (latest.to_string()) }
                            } @else {
                                "N/A"
                            }
                        }
                        td class="has-text-right" {
                            @if dep.is_insecure() {
                                span class="tag is-danger" { "insecure" }
                            } @else if dep.is_outdated() {
                                span class="tag is-warning" { "out of date" }
                            } @else {
                                span class="tag is-success" { "up to date" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn get_site_icon(site: &RepoSite) -> &'static str {
    match *site {
        RepoSite::Github => "github",
        RepoSite::Gitlab => "gitlab",
        RepoSite::Bitbucket => "bitbucket",
    }
}

fn render_title(subject_path: &SubjectPath) -> Markup {
    match *subject_path {
        SubjectPath::Repo(ref repo_path) => {
            let site_icon = get_site_icon(&repo_path.site);
            let fa_site_icon = PreEscaped(fa(FaType::Brands, site_icon).unwrap());

            html! {
                a href=(format!("{}/{}/{}", repo_path.site.to_base_uri(), repo_path.qual.as_ref(), repo_path.name.as_ref())) {
                    { (fa_site_icon) }
                    (format!(" {} / {}", repo_path.qual.as_ref(), repo_path.name.as_ref()))
                }
            }
        }
        SubjectPath::Crate(ref crate_path) => {
            let fa_cube = PreEscaped(fa(FaType::Solid, "cube").unwrap());

            html! {
                a href=(get_crates_version_url(&crate_path.name, &crate_path.version)) {
                    { (fa_cube) }
                    (format!(" {} {}", crate_path.name.as_ref(), crate_path.version))
                }
            }
        }
    }
}

fn render_dev_dependency_box(outcome: &AnalyzeDependenciesOutcome) -> Markup {
    let insecure = outcome.count_dev_insecure();
    let outdated = outcome.count_dev_outdated();
    let text = if insecure > 0 {
        format!("{} insecure development dependencies", insecure)
    } else {
        format!("{} outdated development dependencies", outdated)
    };

    html! {
        div class="notification is-warning" {
            p { "This project contains " b { (text) } "." }
        }
    }
}

fn build_rustsec_link(advisory: &Advisory) -> String {
    format!(
        "https://rustsec.org/advisories/{}.html",
        advisory.id().as_str()
    )
}

fn render_markdown(description: &str) -> Markup {
    let mut rendered = String::new();
    html::push_html(&mut rendered, Parser::new(description));
    PreEscaped(rendered)
}

/// Renders a list of all security vulnerabilities affecting the repository
fn vulnerability_list(analysis_outcome: &AnalyzeDependenciesOutcome) -> Markup {
    let mut vulnerabilities = Vec::new();
    for (_, analyzed_crate) in &analysis_outcome.crates {
        vulnerabilities.extend(
            &mut analyzed_crate
                .main
                .iter()
                .filter(|&(_, dep)| dep.is_insecure())
                .map(|(_, dep)| &dep.vulnerabilities),
        );
        vulnerabilities.extend(
            &mut analyzed_crate
                .dev
                .iter()
                .filter(|&(_, dep)| dep.is_insecure())
                .map(|(_, dep)| &dep.vulnerabilities),
        );
        vulnerabilities.extend(
            &mut analyzed_crate
                .build
                .iter()
                .filter(|&(_, dep)| dep.is_insecure())
                .map(|(_, dep)| &dep.vulnerabilities),
        );
    }

    // flatten Vec<Vec<&Advisory>> -> Vec<&Advisory>
    let mut vulnerabilities: Vec<&Advisory> = vulnerabilities.into_iter().flatten().collect();
    vulnerabilities.sort_unstable_by_key(|&v| v.id());
    vulnerabilities.dedup();

    html! {
        h3 class="title is-3" id="vulnerabilities" { "Security Vulnerabilities" }

        @for vuln in vulnerabilities {
            div class="box" {
                h3 class="title is-4" { code { (vuln.metadata.package.as_str()) } ": " (vuln.title()) }
                p class="subtitle is-5" style="margin-top: -0.5rem;" { a href=(build_rustsec_link(vuln)) { (vuln.id()) } }

                article { (render_markdown(vuln.description())) }

                nav class="level" style="margin-top: 1rem;" {
                    div class="level-item has-text-centered" {
                        div {
                            p class="heading" { "Unaffected" }
                            @if vuln.versions.unaffected.is_empty() {
                                p class="is-grey" { "None"}
                            } @else {
                                @for item in &vuln.versions.unaffected {
                                    p { code { (item) } }
                                }
                            }
                        }
                    }
                    div class="level-item has-text-centered" {
                        div {
                            p class="heading" { "Patched" }
                            @if vuln.versions.unaffected.is_empty() {
                                p class="has-text-grey" { "None"}
                            } @else {
                                @for item in &vuln.versions.patched {
                                    p { code { (item) } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_failure(subject_path: SubjectPath) -> Markup {
    html! {
        section class="hero is-light" {
            div class="hero-head" { (super::render_navbar()) }
            div class="hero-body" {
                div class="container" {
                    h1 class="title is-1" {
                        (render_title(&subject_path))
                    }
                }
            }
        }
        section class="section" {
            div class="container" {
                div class="notification is-danger" {
                    h2 class="title is-3" { "Failed to analyze repository" }
                    p { "The repository you requested might be structured in an uncommon way that is not yet supported." }
                }
            }
        }
        (super::render_footer(None))
    }
}

fn render_success(
    analysis_outcome: AnalyzeDependenciesOutcome,
    subject_path: SubjectPath,
) -> Markup {
    let self_path = match subject_path {
        SubjectPath::Repo(ref repo_path) => format!(
            "repo/{}/{}/{}",
            repo_path.site.as_ref(),
            repo_path.qual.as_ref(),
            repo_path.name.as_ref()
        ),
        SubjectPath::Crate(ref crate_path) => {
            format!("crate/{}/{}", crate_path.name.as_ref(), crate_path.version)
        }
    };
    let status_base_url = format!("{}/{}", &super::SELF_BASE_URL as &str, self_path);

    let status_data_uri = badge::badge(Some(&analysis_outcome)).to_svg_data_uri();

    let hero_class = if analysis_outcome.any_insecure() {
        "is-danger"
    } else if analysis_outcome.any_outdated() {
        "is-warning"
    } else {
        "is-success"
    };

    html! {
        section class=(format!("hero {}", hero_class)) {
            div class="hero-head" { (super::render_navbar()) }
            div class="hero-body" {
                div class="container" {
                    h1 class="title is-1" {
                        (render_title(&subject_path))
                    }

                    img src=(status_data_uri);
                }
            }
            div class="hero-footer" {
                div class="container" {
                    pre class="is-size-7" {
                        (format!("[![dependency status]({}/status.svg)]({})", status_base_url, status_base_url))
                    }
                }
            }
        }
        section class="section" {
            div class="container" {
                @if analysis_outcome.any_insecure() {
                    div class="notification is-warning" {
                        p { "This project contains "
                            b { "known security vulnerabilities" }
                            ". Find detailed information at the "
                            a href="#vulnerabilities" { "bottom"} "."
                        }
                    }
                } @else if analysis_outcome.any_dev_issues() {
                    (render_dev_dependency_box(&analysis_outcome))
                }
                @for (crate_name, deps) in &analysis_outcome.crates {
                    (dependency_tables(crate_name, deps))
                }

                @if analysis_outcome.any_insecure() {
                    (vulnerability_list(&analysis_outcome))
                }
            }
        }
        (super::render_footer(Some(analysis_outcome.duration)))
    }
}

pub fn render(
    analysis_outcome: Option<AnalyzeDependenciesOutcome>,
    subject_path: SubjectPath,
) -> Response<Body> {
    let title = match subject_path {
        SubjectPath::Repo(ref repo_path) => {
            format!("{} / {}", repo_path.qual.as_ref(), repo_path.name.as_ref())
        }
        SubjectPath::Crate(ref crate_path) => {
            format!("{} {}", crate_path.name.as_ref(), crate_path.version)
        }
    };

    if let Some(outcome) = analysis_outcome {
        super::render_html(&title, render_success(outcome, subject_path))
    } else {
        super::render_html(&title, render_failure(subject_path))
    }
}
