use hyper::Response;
use maud::{Markup, html};
use indexmap::IndexMap;

use ::engine::AnalyzeDependenciesOutcome;
use ::models::crates::{CrateName, AnalyzedDependency, AnalyzedDependencies};
use ::models::SubjectPath;
use ::models::repo::RepoSite;

use super::super::badge;

fn dependency_tables(crate_name: CrateName, deps: AnalyzedDependencies) -> Markup {
    html! {
        h2 class="title is-3" {
            "Crate "
            code (crate_name.as_ref())
        }

        @if deps.main.is_empty() && deps.dev.is_empty() && deps.build.is_empty() {
            p class="notification has-text-centered" "No external dependencies! 🙌"
        }

        @if !deps.main.is_empty() {
            (dependency_table("Dependencies", deps.main))
        }

        @if !deps.dev.is_empty() {
            (dependency_table("Dev dependencies", deps.dev))
        }

        @if !deps.build.is_empty() {
            (dependency_table("Build dependencies", deps.build))
        }
    }
}

fn dependency_table(title: &str, deps: IndexMap<CrateName, AnalyzedDependency>) -> Markup {
    let count_total = deps.len();
    let count_insecure = deps.iter().filter(|&(_, dep)| dep.insecure).count();
    let count_outdated = deps.iter().filter(|&(_, dep)| dep.is_outdated()).count();

    html! {
        h3 class="title is-4" (title)
        p class="subtitle is-5" {
            @if count_insecure > 0 {
                (format!(" ({} total, {} insecure)", count_total, count_insecure))
            } @else if count_outdated > 0 {
                (format!(" ({} total, {} up-to-date, {} outdated)", count_total, count_total - count_outdated, count_outdated))
            } @else {
                (format!(" ({} total, all up-to-date)", count_total))
            }
        }

        table class="table is-fullwidth is-striped is-hoverable" {
            thead {
                tr {
                    th "Crate"
                    th class="has-text-right" "Required"
                    th class="has-text-right" "Latest"
                    th class="has-text-right" "Status"
                }
            }
            tbody {
                @for (name, dep) in deps {
                    tr {
                        td {
                            a href=(format!("https://crates.io/crates/{}", name.as_ref())) (name.as_ref())
                        }
                        td class="has-text-right" code (dep.required.to_string())
                        td class="has-text-right" {
                            @if let Some(ref latest) = dep.latest {
                                code (latest.to_string())
                            } @else {
                                "N/A"
                            }
                        }
                        td class="has-text-right" {
                            @if dep.insecure {
                                span class="tag is-danger" "insecure"
                            } @else if dep.is_outdated() {
                                span class="tag is-warning" "out of date"
                            } @else {
                                span class="tag is-success" "up to date"
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
        RepoSite::Github => "fa-github",
        RepoSite::Gitlab => "fa-gitlab",
        RepoSite::Bitbucket => "fa-bitbucket"
    }
}

fn render_title(subject_path: &SubjectPath) -> Markup {
    match *subject_path {
        SubjectPath::Repo(ref repo_path) => {
            let site_icon = get_site_icon(&repo_path.site);
            html! {
                a href=(format!("{}/{}/{}", repo_path.site.to_base_uri(), repo_path.qual.as_ref(), repo_path.name.as_ref())) {
                    i class=(format!("fa {}", site_icon)) ""
                    (format!(" {} / {}", repo_path.qual.as_ref(), repo_path.name.as_ref()))
                }
            }
        },
        SubjectPath::Crate(ref crate_path) => {
            html! {
                a href=(format!("https://crates.io/crates/{}/{}", crate_path.name.as_ref(), crate_path.version)) {
                    i class="fa fa-cube" ""
                    (format!(" {} {}", crate_path.name.as_ref(), crate_path.version))
                }
            }
        }
    }
}

fn render_failure(subject_path: SubjectPath) -> Markup {
    html! {
        section class="hero is-light" {
            div class="hero-head" (super::render_navbar())
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
                    h2 class="title is-3" "Failed to analyze repository"
                    p "The repository you requested might be structured in an uncommon way that is not yet supported."
                }
            }
        }
        (super::render_footer(None))
    }
}

fn render_success(analysis_outcome: AnalyzeDependenciesOutcome, subject_path: SubjectPath) -> Markup {
    let self_path = match subject_path {
        SubjectPath::Repo(ref repo_path) =>
            format!("repo/{}/{}/{}", repo_path.site.as_ref(), repo_path.qual.as_ref(), repo_path.name.as_ref()),
        SubjectPath::Crate(ref crate_path) =>
            format!("crate/{}/{}", crate_path.name.as_ref(), crate_path.version)
    };
    let status_base_url = format!("{}/{}", &super::SELF_BASE_URL as &str, self_path);

    let status_data_uri = badge::badge(Some(&analysis_outcome)).to_svg_data_uri();

    let hero_class = if analysis_outcome.any_insecure()  {
        "is-danger"
    } else if analysis_outcome.any_outdated() {
        "is-warning"
    } else {
        "is-success"
    };

    html! {
        section class=(format!("hero {}", hero_class)) {
            div class="hero-head" (super::render_navbar())
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
                @for (crate_name, deps) in analysis_outcome.crates {
                    (dependency_tables(crate_name, deps))
                }
            }
        }
        (super::render_footer(Some(analysis_outcome.duration)))
    }
}

pub fn render(analysis_outcome: Option<AnalyzeDependenciesOutcome>, subject_path: SubjectPath) -> Response {
    let title = match subject_path {
        SubjectPath::Repo(ref repo_path) =>
            format!("{} / {}", repo_path.qual.as_ref(), repo_path.name.as_ref()),
        SubjectPath::Crate(ref crate_path) =>
            format!("{} {}", crate_path.name.as_ref(), crate_path.version)
    };

    if let Some(outcome) = analysis_outcome {
        super::render_html(&title, render_success(outcome, subject_path))
    } else {
        super::render_html(&title, render_failure(subject_path))
    }
}
