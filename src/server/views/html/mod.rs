use std::env;

use hyper::Response;
use hyper::header::ContentType;
use maud::{Markup, Render, html};

pub mod index;
pub mod status;

lazy_static! {
    static ref SELF_BASE_URL: String = {
        env::var("BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string())
    };
}

fn render_html<B: Render>(title: &str, body: B) -> Response {
    let rendered = html! {
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title (title)
                link rel="icon" type="image/png" href="/static/favicon.png";
                link rel="stylesheet" type="text/css" href="/static/style.css";
                link rel="stylesheet" type="text/css" href="https://fonts.googleapis.com/css?family=Fira+Sans:400,500,600";
                link rel="stylesheet" type="text/css" href="https://fonts.googleapis.com/css?family=Source+Code+Pro";
                link rel="stylesheet" type="text/css" href="https://maxcdn.bootstrapcdn.com/font-awesome/4.7.0/css/font-awesome.min.css";
            }
            body {
                (body)
            }
        }
    };

    Response::new()
        .with_header(ContentType::html())
        .with_body(rendered.0)
}

fn render_navbar() -> Markup {
    html! {
        header class="navbar" {
            div class="container" {
                div class="navbar-brand" {
                    a class="navbar-item is-dark" href=(SELF_BASE_URL) {
                        h1 class="title is-3" "Deps.rs"
                    }
                }
            }
        }
    }
}
