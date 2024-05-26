use actix_http::{body::MessageBody, Response};
use font_awesome_as_a_crate::{svg as fa, Type as FaType};
use indexmap::IndexMap;
use maud::{html, Markup, PreEscaped};
use pulldown_cmark::{html, Parser};
use rustsec::advisory::Advisory;
use semver::Version;

use crate::{
    engine::AnalyzeDependenciesOutcome,
    models::{
        crates::{AnalyzedDependencies, AnalyzedDependency, CrateName},
        repo::RepoSite,
        SubjectPath,
    },
    server::{views::badge, ExtraConfig},
};

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
    let count_always_insecure = deps
        .iter()
        .filter(|&(_, dep)| dep.is_always_insecure())
        .count();
    let count_insecure = deps.iter().filter(|&(_, dep)| dep.is_insecure()).count();
    let count_outdated = deps.iter().filter(|&(_, dep)| dep.is_outdated()).count();

    let fa_cube = PreEscaped(fa(FaType::Solid, "cube").unwrap());

    html! {
        h3 class="title is-4" { (title) }
        p class="subtitle is-5" {
            (match (count_outdated, count_always_insecure, count_insecure - count_always_insecure) {
                (0, 0, 0) => format!("({count_total} total, all up-to-date)"),
                (0, 0, c) => format!("({count_total} total, {c} possibly insecure)"),
                (_, 0, 0) => format!("({count_total} total, {count_outdated} outdated)"),
                (0, _, 0) => format!("({count_total} total, {count_always_insecure} insecure)"),
                (0, _, c) => format!("({count_total} total, {count_always_insecure} insecure, {c} possibly insecure)"),
                (_, 0, c) => format!("({count_total} total, {count_outdated} outdated, {c} possibly insecure)"),
                (_, _, 0) => format!("({count_total} total, {count_outdated} outdated, {count_always_insecure} insecure)"),
                (_, _, c) => format!("({count_total} total, {count_outdated} outdated, {count_always_insecure} insecure, {c} possibly insecure)"),
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
                            a class="has-text-grey" href=(get_crates_url(name)) {
                                { (fa_cube) }
                            }
                            { "\u{00A0}" } // non-breaking space
                            a href=(dep.deps_rs_path(name.as_ref())) { (name.as_ref()) }

                            @if dep.is_insecure() {
                                { "\u{00A0}" } // non-breaking space
                                a href="#vulnerabilities" title="has known vulnerabilities" { "⚠️" }
                            }
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
                            @if dep.is_always_insecure() {
                                span class="tag is-danger" { "insecure" }
                            } @else if dep.is_outdated() {
                                span class="tag is-warning" { "out of date" }
                            } @else if dep.is_insecure() {
                                span class="tag is-warning" { "maybe insecure" }
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

fn get_site_icon(site: &RepoSite) -> (FaType, &'static str) {
    match *site {
        RepoSite::Github => (FaType::Brands, "github"),
        RepoSite::Gitlab => (FaType::Brands, "gitlab"),
        RepoSite::Bitbucket => (FaType::Brands, "bitbucket"),
        // FIXME: There is no brands/{sourcehut, codeberg, gitea} icon, so just use an
        // icon which looks close enough.
        RepoSite::Sourcehut => (FaType::Regular, "circle"),
        RepoSite::Codeberg => (FaType::Solid, "mountain"),
        RepoSite::Gitea(_) => (FaType::Brands, "git-alt"),
    }
}

fn render_title(subject_path: &SubjectPath) -> Markup {
    match *subject_path {
        SubjectPath::Repo(ref repo_path) => {
            let site_icon = get_site_icon(&repo_path.site);
            let fa_site_icon = PreEscaped(fa(site_icon.0, site_icon.1).unwrap());

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

/// Renders a path within a repository as HTML.
///
/// Panics, when the string is empty.
fn render_path(inner_path: &str) -> Markup {
    let path_icon = PreEscaped(fa(FaType::Regular, "folder-open").unwrap());

    let mut splitted = inner_path.trim_matches('/').split('/');
    let init = splitted.next().unwrap().to_string();
    let path_spaced = splitted.fold(init, |b, val| b + " / " + val);

    html! {
        { (path_icon) }
        " / "
        (path_spaced)
    }
}

fn dependencies_pluralized(count: usize) -> &'static str {
    if count == 1 {
        "dependency"
    } else {
        "dependencies"
    }
}

/// Renders a warning with the numbers of outdated dependencies (of both kinds)
/// or insecure dev-dependencies.
///
/// The function assumes that there are no insecure main dependencies.
/// If there is more than one kind of dependency with issues,
/// an unordered list is rendered.
/// Renders nothing if the counts of all three components are zero.
fn render_dependency_box(outcome: &AnalyzeDependenciesOutcome) -> Markup {
    // assuming zero insecure main dependencies
    let insecure_dev = outcome.count_dev_insecure();
    let outdated_dev = outcome.count_dev_outdated();
    let outdated = outcome.count_outdated();

    let components = [
        ("insecure development", insecure_dev),
        ("outdated main", outdated),
        ("outdated development", outdated_dev),
    ]
    .iter()
    .copied()
    .filter(|&(_, c)| c > 0)
    .map(|(kind, c)| {
        let pluralized = dependencies_pluralized(c);
        (c, kind, pluralized)
    })
    .collect::<Vec<_>>();

    match components.len() {
        0 => html! {},
        1 => {
            let (c, kind, dep) = components[0];
            html! {
                div class="notification is-warning" {
                    p { "This project contains " b { (c) " " (kind) " " (dep) } "." }
                }
            }
        }
        _ => html! {
            div class="notification is-warning" {
                p { "This project contains:" }
                ul {
                    @for (c, kind, dep) in components {
                        li { b { (c) " " (kind) " " (dep) } }
                    }
                }
            }
        },
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
                p class="subtitle is-5" style="margin-top: -0.5rem;" { a href=(build_rustsec_link(vuln)) { (vuln.id().to_string()) } }

                article { (render_markdown(vuln.description())) }

                nav class="level" style="margin-top: 1rem;" {
                    div class="level-item has-text-centered" {
                        div {
                            p class="heading" { "Unaffected" }
                            @if vuln.versions.unaffected().is_empty() {
                                p class="is-grey" { "None"}
                            } @else {
                                @for item in vuln.versions.unaffected() {
                                    p { code { (item.to_string()) } }
                                }
                            }
                        }
                    }
                    div class="level-item has-text-centered" {
                        div {
                            p class="heading" { "Patched" }
                            @if vuln.versions.patched().is_empty() {
                                p class="has-text-grey" { "None"}
                            } @else {
                                @for item in vuln.versions.patched() {
                                    p { code { (item.to_string()) } }
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
    extra_config: ExtraConfig,
) -> Markup {
    let self_path = match subject_path {
        SubjectPath::Repo(ref repo_path) => format!(
            "repo/{}/{}/{}",
            repo_path.site,
            repo_path.qual.as_ref(),
            repo_path.name.as_ref()
        ),
        SubjectPath::Crate(ref crate_path) => {
            format!("crate/{}/{}", crate_path.name.as_ref(), crate_path.version)
        }
    };
    let status_base_url = format!("{}/{}", &super::SELF_BASE_URL as &str, self_path);

    let status_data_uri =
        badge::badge(Some(&analysis_outcome), extra_config.clone()).to_svg_data_uri();

    let hero_class = if analysis_outcome.any_always_insecure() {
        "is-danger"
    } else if analysis_outcome.any_insecure() || analysis_outcome.any_outdated() {
        "is-warning"
    } else {
        "is-success"
    };

    // NOTE(feliix42): While we could encode the whole `ExtraConfig` struct here, I've decided
    // against doing so as this would always append the defaults for badge style and compactness
    // settings to the URL, bloating it unnecessarily, we can do that once it's needed.
    let options = serde_urlencoded::to_string([(
        "path",
        extra_config.path.clone().unwrap_or_default().as_str(),
    )])
    .unwrap();

    html! {
        section class=(format!("hero {hero_class}")) {
            div class="hero-head" { (super::render_navbar()) }
            div class="hero-body" {
                div class="container" {
                    h1 class="title is-1" {
                        (render_title(&subject_path))
                    }

                    @if let Some(ref path) = extra_config.path {
                        p class="subtitle" {
                            (render_path(path))
                        }
                    }

                    img src=(status_data_uri);
                }
            }
            div class="hero-footer" {
                div class="container" {
                    pre class="is-size-7" {
                        @if extra_config.path.is_some() {
                            (format!("[![dependency status]({status_base_url}/status.svg?{options})]({status_base_url}?{options})"))
                        } @else {
                            (format!("[![dependency status]({status_base_url}/status.svg)]({status_base_url})"))
                        }
                    }
                }
            }
        }
        section class="section" {
            div class="container" {
                @if analysis_outcome.any_always_insecure() {
                    div class="notification is-warning" {
                        p { "This project contains "
                            b { "known security vulnerabilities" }
                            ". Find detailed information at the "
                            a href="#vulnerabilities" { "bottom"} "."
                        }
                    }
                } @else if analysis_outcome.any_insecure() {
                    div class="notification is-warning" {
                        p { "This project might be open to "
                            b { "known security vulnerabilities" }
                            ", which can be prevented by tightening "
                            "the version range of affected dependencies. "
                            "Find detailed information at the "
                            a href="#vulnerabilities" { "bottom"} "."
                        }
                    }
                } @else {
                    (render_dependency_box(&analysis_outcome))
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
    extra_config: ExtraConfig,
) -> Response<impl MessageBody> {
    let title = match subject_path {
        SubjectPath::Repo(ref repo_path) => {
            format!("{} / {}", repo_path.qual.as_ref(), repo_path.name.as_ref())
        }
        SubjectPath::Crate(ref crate_path) => {
            format!("{} {}", crate_path.name.as_ref(), crate_path.version)
        }
    };

    if let Some(outcome) = analysis_outcome {
        super::render_html(&title, render_success(outcome, subject_path, extra_config))
    } else {
        super::render_html(&title, render_failure(subject_path))
    }
}
