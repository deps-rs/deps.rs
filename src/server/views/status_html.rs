use std::collections::BTreeMap;
use std::env;

use base64::display::Base64Display;
use hyper::Response;
use hyper::header::ContentType;
use maud::{Markup, html};

use ::engine::AnalyzeDependenciesOutcome;
use ::models::crates::{CrateName, AnalyzedDependency};
use ::models::repo::RepoPath;
use ::server::assets;

lazy_static! {
    static ref SELF_BASE_URL: String = {
        env::var("BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string())
    };
}

fn dependency_table(title: &str, deps: BTreeMap<CrateName, AnalyzedDependency>) -> Markup {
    let count_total = deps.len();
    let count_outdated = deps.iter().filter(|&(_, dep)| dep.is_outdated()).count();

    html! {
        h3 class="title is-4" (title)
        p class="subtitle is-5" {
            @if count_outdated > 0 {
                (format!(" ({} total, {} up-to-date, {} outdated)", count_total, count_total - count_outdated, count_outdated))
            } @else {
                (format!(" ({} total, all up-to-date)", count_total))
            }
        }

        table class="table is-fullwidth is-striped is-hoverable" {
            thead {
                tr {
                    th "Crate"
                    th "Required"
                    th "Latest"
                    th "Status"
                }
            }
            tbody {
                @for (name, dep) in deps {
                    tr {
                        td {
                            a href=(format!("https://crates.io/crates/{}", name.as_ref())) (name.as_ref())
                        }
                        td code (dep.required.to_string())
                        td {
                            @if let Some(ref latest) = dep.latest {
                                code (latest.to_string())
                            } @else {
                                "N/A"
                            }
                        }
                        td {
                            @if dep.is_outdated() {
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

pub fn status_html(analysis_outcome: AnalyzeDependenciesOutcome, repo_path: RepoPath) -> Response {
    let self_path = format!("repo/{}/{}/{}", repo_path.site.as_ref(), repo_path.qual.as_ref(), repo_path.name.as_ref());
    let status_base_url = format!("{}/{}", &SELF_BASE_URL as &str, self_path);
    let title = format!("{} / {} - Dependency Status", repo_path.qual.as_ref(), repo_path.name.as_ref());

    let (hero_class, status_asset) = if analysis_outcome.deps.any_outdated() {
        ("is-warning", assets::BADGE_OUTDATED_SVG.as_ref())
    } else {
        ("is-success", assets::BADGE_UPTODATE_SVG.as_ref())
    };

    let status_data_url = format!("data:image/svg+xml;base64,{}", Base64Display::standard(status_asset));

    let rendered = html! {
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title (title)
                link rel="stylesheet" type="text/css" href="/static/style.css";
                link rel="stylesheet" type="text/css" href="https://fonts.googleapis.com/css?family=Fira+Sans:400,500,600";
                link rel="stylesheet" type="text/css" href="https://fonts.googleapis.com/css?family=Source+Code+Pro";
                link rel="stylesheet" type="text/css" href="https://maxcdn.bootstrapcdn.com/font-awesome/4.7.0/css/font-awesome.min.css";
            }
            body {
                section class=(format!("hero {}", hero_class)) {
                    div class="hero-body" {
                        div class="container" {
                            h1 class="title is-1" {
                                a href=(format!("{}/{}/{}", repo_path.site.to_base_uri(), repo_path.qual.as_ref(), repo_path.name.as_ref())) {
                                    i class="fa fa-github" ""
                                    (format!(" {} / {}", repo_path.qual.as_ref(), repo_path.name.as_ref()))
                                }
                            }

                            img src=(status_data_url);
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
                        h2 class="title is-3" {
                            "Crate "
                            code (analysis_outcome.name.as_ref())
                        }

                        @if !analysis_outcome.deps.main.is_empty() {
                            (dependency_table("Dependencies", analysis_outcome.deps.main))
                        }

                        @if !analysis_outcome.deps.dev.is_empty() {
                            (dependency_table("Dev dependencies", analysis_outcome.deps.dev))
                        }

                        @if !analysis_outcome.deps.build.is_empty() {
                            (dependency_table("Build dependencies", analysis_outcome.deps.build))
                        }
                    }
                }
            }
        }
    };

    Response::new()
        .with_header(ContentType::html())
        .with_body(rendered.0)
}
