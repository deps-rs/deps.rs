use hyper::Response;
use maud::{Markup, html};

use ::models::repo::Repository;

fn popular_table(popular: Vec<Repository>) -> Markup {
    html! {
        h2 class="title is-3" "Popular Repositories"

        table class="table is-fullwidth is-striped is-hoverable" {
            thead {
                tr {
                    th "Repository"
                    th class="has-text-right" "Status"
                }
            }
            tbody {
                @for repo in popular {
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
}

pub fn render(popular: Vec<Repository>) -> Response {
    super::render_html("Keep your dependencies up-to-date", html! {
        section class="hero is-light" {
            div class="hero-head" (super::render_navbar())
            div class="hero-body" {
                div class="container" {
                    p class="title is-1" "Keep your dependencies up-to-date"
                    p {
                        "Docs.rs uses semantic versioning to detect outdated or insecure dependencies in your project's"
                        code "Cargo.toml"
                        "."
                    }
                }
            }
        }
        section class="section" {
            div class="container" (popular_table(popular))
        }
    })
}
