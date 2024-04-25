use maud::{html, DOCTYPE};

pub fn maud_page(content: maud::Markup) -> maud::Markup {
    html! {
       (DOCTYPE)
       (maud_header())
       (maud_body(content))
    }
}

fn maud_header() -> maud::Markup {
    html! {
        link rel="stylesheet" href="static/tailwind.css";
        link rel="stylesheet" href="static/quill/snow.css";
        script src="/static/quill/quill.js" {}
        meta name="viewport" content="width=device-width, initial-scale=1.0";
    }
}

fn maud_nav() -> maud::Markup {
    html! {
        nav class="nav bg-gray-100" {

            div class="flex lg:flex-1 m-1" {
                a class="no-underline hover:no-underline font-extrabold m-3 text-2xl" href="/" { "W2Z" }

            };
        }
    }
}

fn maud_body(content: maud::Markup) -> maud::Markup {
    html! {
        body {
            (maud_nav())
            div class="w-full lg:max-w-3xl mx-auto pt-20 lg:place-content-center" {

                div class="w-full px-2 lg:px-6 leading-normal" {
                        (content)
                };
            };
        };
    }
}
