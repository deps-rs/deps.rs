use hyper::Response;
use maud::{Markup, html};

use ::models::repo::Repository;
use ::models::crates::CratePath;

fn popular_table(popular_repos: Vec<Repository>, popular_crates: Vec<CratePath>) -> Markup {
    html! {
        div class="columns" {
            div class="column" {
                h2 class="title is-3" { "Popular Repositories" }

                table class="table is-fullwidth is-striped is-hoverable" {
                    thead {
                        tr {
                            th { "Repository" }
                            th class="has-text-right" { "Status" }
                        }
                    }
                    tbody {
                        @for repo in popular_repos.into_iter().take(10) {
                            tr {
                                td {
                                    a href=(format!("{}/repo/{}/{}/{}", &super::SELF_BASE_URL as &str, repo.path.site.as_ref(), repo.path.qual.as_ref(), repo.path.name.as_ref())) {
                                        (format!("{} / {}", repo.path.qual.as_ref(), repo.path.name.as_ref()))
                                    }
                                }
                                td class="has-text-right" {
                                    img src=(format!("{}/repo/{}/{}/{}/status.svg", &super::SELF_BASE_URL as &str, repo.path.site.as_ref(), repo.path.qual.as_ref(), repo.path.name.as_ref()));
                                }
                            }
                        }
                    }
                }
            }
            div class="column" {
                h2 class="title is-3" { "Popular Crates" }

                table class="table is-fullwidth is-striped is-hoverable" {
                    thead {
                        tr {
                            th { "Crate" }
                            th class="has-text-right" { "Status" }
                        }
                    }
                    tbody {
                        @for crate_path in popular_crates {
                            tr {
                                td {
                                    a href=(format!("{}/crate/{}/{}", &super::SELF_BASE_URL as &str, crate_path.name.as_ref(), crate_path.version)) {
                                        (format!("{}", crate_path.name.as_ref()))
                                    }
                                }
                                td class="has-text-right" {
                                    img src=(format!("{}/crate/{}/{}/status.svg", &super::SELF_BASE_URL as &str, crate_path.name.as_ref(), crate_path.version));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn render(popular_repos: Vec<Repository>, popular_crates: Vec<CratePath>) -> Response {
    super::render_html("Keep your dependencies up-to-date", html! {
        section class="hero is-light" {
            div class="hero-head" { (super::render_navbar()) }
            div class="hero-body" {
                div class="container" {
                    p class="title is-1" { "Keep your dependencies up-to-date" }
                    p {
                        "Deps.rs uses semantic versioning to detect outdated or insecure dependencies in your project's"
                        code { "Cargo.toml" }
                        "."
                    }
                }
            }
        }
        section class="section" {
            div class="container" { (popular_table(popular_repos, popular_crates)) }
        }
        (super::render_footer(None))
    })
}
