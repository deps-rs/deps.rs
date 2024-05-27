use axum::{body::Body, response::Response};
use maud::{html, Markup};

use crate::{
    models::{crates::CratePath, repo::Repository},
    server::assets::STATIC_LINKS_JS_PATH,
};

fn link_forms() -> Markup {
    html! {
        div class="columns" {
            div class="column" {
                div class="box" {
                    h2 class="title c is-3" { "Check a Repository" }

                    form id="repoSelect" action="#" {
                        div class="field" {
                            label class="label" { "Hosting Provider" }

                            div class="control" {
                                div class="select" {
                                    select id="hosterSelect" {
                                        option { "Github" }
                                        option { "Gitlab" }
                                        option { "Bitbucket" }
                                        option { "Sourcehut" }
                                        option { "Codeberg" }
                                        option { "Gitea" }
                                    }
                                }
                            }
                        }

                        div class="field" {
                            label class="label" { "Owner" }

                            div class="control" {
                                input class="input" type="text" id="owner" placeholder="rust-lang" required;
                            }
                        }

                        div class="field" {
                            label class="label" { "Repository Name" }

                            div class="control" {
                                input class="input" type="text" id="repoName" placeholder="cargo" required;
                            }
                        }

                        div class="field" {
                            label class="label" { "Git instance URL" }

                            div class="control" {
                                input class="input" type="text" id="baseUrl" placeholder="gitea.com";
                            }

                            p class="help" id="baseUrlHelp" { "Base URL of the Git instance the project is hosted on. Only relevant for Gitea Instances." }
                        }

                        div class="field" {
                            label class="label" { "Path in Repository" }

                            div class="control" {
                                input class="input" type="text" id="innerPath" placeholder="project1/rust-stuff";
                            }

                            p class="help" id="baseUrlHelp" { "Path within the repository where the " code { "Cargo.toml" } " file is located." }
                        }

                        input type="submit" class="button is-primary" value="Check" onclick="return buildRepoLink();";
                    }
                }
            }
            div class="column" {
                div class="box" {
                    h2 class="title is-3" { "Check a Crate" }

                    form id="crateSelect" action="#" {
                        div class="field" {
                            label class="label" { "Crate Name" }

                            div class="control" {
                                input class="input" type="text" id="crateName" placeholder="serde" required;
                            }
                        }

                        div class="field" {
                            label class="label" { "Version (optional)" }

                            div class="control" {
                                input class="input" type="text" id="crateVersion" placeholder="1.0.0";
                            }

                            p class="help" { "If left blank, defaults to the latest version." }
                        }

                        input type="submit" class="button is-primary" value="Check" onclick="return buildCrateLink();";
                    }
                }
            }
        }
    }
}

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
                                    a href=(format!("{}/repo/{}/{}/{}", &super::SELF_BASE_URL as &str, repo.path.site, repo.path.qual.as_ref(), repo.path.name.as_ref())) {
                                        (format!("{} / {}", repo.path.qual.as_ref(), repo.path.name.as_ref()))
                                    }
                                }
                                td class="has-text-right" {
                                    img src=(format!("{}/repo/{}/{}/{}/status.svg", &super::SELF_BASE_URL as &str, repo.path.site, repo.path.qual.as_ref(), repo.path.name.as_ref()));
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
                                        (crate_path.name.as_ref().to_string())
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

pub fn render(popular_repos: Vec<Repository>, popular_crates: Vec<CratePath>) -> Response<Body> {
    super::render_html(
        "Keep your dependencies up-to-date",
        html! {
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
            section class="section" {
                div class="container" { (link_forms()) }
            }
            (super::render_footer(None))
            script src=(STATIC_LINKS_JS_PATH) {}
        },
    )
}
