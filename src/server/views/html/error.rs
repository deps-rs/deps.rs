use hyper::{
    header::{CACHE_CONTROL, CONTENT_TYPE},
    Body, Response, StatusCode,
};
use maud::html;

use crate::server::assets::STATIC_STYLE_CSS_PATH;

pub fn render(title: &str, descr: &str) -> Response<Body> {
    super::render_html(
        title,
        html! {
            section class="hero is-light" {
                div class="hero-head" { (super::render_navbar()) }
            }
            section class="section" {
                div class="container" {
                    div class="notification is-danger" {
                        p class="title is-3" { (title) }
                        p { (descr) }
                    }
                }
            }
            (super::render_footer(None))
        },
    )
}

pub fn render_404() -> Response<Body> {
    let rendered = html! {
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "404 - Deps.rs" }
                link rel="icon" type="image/svg+xml" href="/static/logo.svg";
                link rel="stylesheet" type="text/css" href=(STATIC_STYLE_CSS_PATH);
                link rel="stylesheet" type="text/css" href="https://fonts.googleapis.com/css?family=Fira+Sans:400,500,600";
                link rel="stylesheet" type="text/css" href="https://fonts.googleapis.com/css?family=Source+Code+Pro";
            }
            body {
                section class="hero is-light" {
                    div class="hero-head" { (super::render_navbar()) }
                }
                section class="section" {
                    div class="container" {
                        div class="notification is-info" {
                            p class="title is-3" { "Ooops, seems like you've hit a dead end!" }
                            p { "The page you were looking for could not be found. In other words, this is a " b { "404 error" } "." }
                        }
                    }
                }
                (super::render_footer(None))
            }
        }
    };

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .header(CACHE_CONTROL, "public, max-age=300, immutable")
        .body(Body::from(rendered.0))
        .unwrap()
}
