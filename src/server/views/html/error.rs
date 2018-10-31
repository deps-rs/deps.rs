use hyper::{Body, Response};
use maud::html;

pub fn render(title: &str, descr: &str) -> Response<Body> {
    super::render_html(title, html! {
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
    })
}
