use hyper::Response;
use maud::html;

pub fn render(title: &str, descr: &str) -> Response { 
    super::render_html(title, html! {
        section class="hero is-light" {
            div class="hero-head" (super::render_navbar())
            div class="hero-body" {
                div class="container" {
                    p class="title is-1" (title)
                    p (descr)
                }
            }
        }
    })
}
