use std::time::Duration;

use hyper::header::CONTENT_TYPE;
use hyper::{Body, Response};
use maud::{html, Markup, Render};

pub mod error;
pub mod index;
pub mod status;

use crate::server::assets::STATIC_STYLE_CSS_PATH;
use crate::server::SELF_BASE_URL;

fn render_html<B: Render>(title: &str, body: B) -> Response<Body> {
    let rendered = html! {
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (format!("{} - Deps.rs", title)) }
                link rel="icon" type="image/svg+xml" href="/static/logo.svg";
                link rel="stylesheet" type="text/css" href=(STATIC_STYLE_CSS_PATH);
                link rel="stylesheet" type="text/css" href="https://fonts.googleapis.com/css?family=Fira+Sans:400,500,600";
                link rel="stylesheet" type="text/css" href="https://fonts.googleapis.com/css?family=Source+Code+Pro";
                link rel="stylesheet" type="text/css" href="https://maxcdn.bootstrapcdn.com/font-awesome/4.7.0/css/font-awesome.min.css";
            }
            body { (body) }
        }
    };

    Response::builder()
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(rendered.0))
        .unwrap()
}

fn render_navbar() -> Markup {
    html! {
        header class="navbar" {
            div class="container" {
                div class="navbar-brand" {
                    a class="navbar-item is-dark" href=(SELF_BASE_URL.as_str()) {
                        h1 class="title is-3" { "Deps.rs" }
                    }
                }
            }
        }
    }
}

fn render_footer(duration: Option<Duration>) -> Markup {
    let duration_millis = duration.map(|d| d.as_secs() * 1000 + (d.subsec_millis()) as u64);

    html! {
        footer class="footer" {
            div class="container" {
                div class="content has-text-centered" {
                    p {
                        strong { "Deps.rs" }
                        " is a service for the Rust community. It is open source on "
                        a href="https://github.com/deps-rs/deps.rs" { "GitHub" }
                        "."
                    }
                    p {
                        "Please report any issues on the "
                        a href="https://github.com/deps-rs/deps.rs/issues" { "issue tracker" }
                        "."
                    }
                    @if let Some(millis) = duration_millis {
                        p class="has-text-grey is-size-7" { (format!("(rendered in {} ms)", millis)) }
                    }
                }
            }
        }
    }
}
