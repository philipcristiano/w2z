use axum::{
    extract::{FromRef, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use clap::Parser;
use maud::html;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;

use tower_cookies::CookieManagerLayer;

pub mod backends;
mod html;
mod templating;

use rust_embed::RustEmbed;
#[derive(RustEmbed, Clone)]
#[folder = "static/"]
struct StaticAssets;

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long, default_value = "127.0.0.1:3000")]
    bind_addr: String,
    #[arg(short, long, default_value = "w2z.toml")]
    config_file: String,
    #[arg(short, long, value_enum, default_value = "INFO")]
    log_level: tracing::Level,
    #[arg(long, action)]
    log_json: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct AppConfig {
    auth: service_conventions::oidc::OIDCConfig,
    github: backends::github::GithubConfig,
}

#[derive(FromRef, Clone, Debug)]
pub struct AppState {
    auth: service_conventions::oidc::AuthConfig,
    backend: backends::github::GithubBackend,
    //templates: tera::Tera,
}

impl From<AppConfig> for AppState {
    fn from(item: AppConfig) -> Self {
        let auth_config = service_conventions::oidc::AuthConfig {
            oidc_config: item.auth,
            post_auth_path: "/".to_string(),
            scopes: vec!["profile".to_string(), "email".to_string()],
        };
        AppState {
            auth: auth_config,
            backend: backends::github::GithubBackend::from_config(item.github),
            //templates: templating::create_tera(&item.templates),
        }
    }
}

#[tokio::main]
async fn main() {
    // initialize tracing

    let args = Args::parse();

    service_conventions::tracing::setup(args.log_level);

    let config_file_error_msg = format!("Could not read config file {}", args.config_file);
    let config_file_contents = fs::read_to_string(args.config_file).expect(&config_file_error_msg);

    let app_config: AppConfig =
        toml::from_str(&config_file_contents).expect("Problems parsing config file");

    tracing::debug!("Config {:?}", app_config);
    let app_state: AppState = app_config.into();

    let oidc_router = service_conventions::oidc::router(app_state.auth.clone());

    let serve_assets = axum_embed::ServeEmbed::<StaticAssets>::new();
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        //.route("/notes", post(post_note))
        .route(
            "/b/github/{owner}/{repo}",
            get(backends::github::axum_get_site),
        )
        .route(
            "/b/github/{owner}/{repo}/new/{template_name}",
            post(backends::github::axum_post_template),
        )
        //.route("/replies", post(post_reply))
        //.route("/likes", post(post_like))
        .nest("/oidc", oidc_router.with_state(app_state.auth.clone()))
        .with_state(app_state.clone())
        .nest_service("/static", serve_assets)
        .layer(CookieManagerLayer::new())
        .layer(service_conventions::tracing_http::trace_layer(
            tracing::Level::INFO,
        ))
        .route("/_health", get(health));

    let addr: SocketAddr = args.bind_addr.parse().expect("Expected bind addr");
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Response {
    "OK".into_response()
}

// basic handler that responds with a static string
async fn root(
    State(app_state): State<AppState>,
    user: Option<service_conventions::oidc::OIDCUser>,
) -> Result<Response, AppError> {
    if let Some(user) = user {
        let sites = &app_state.backend.sites().await?;
        tracing::debug!(sites= ?sites, "Sites");

        Ok(html::maud_page(html! {
              @for site in sites {
                  p {
                    a href={"/b/github/" (site.name)}
                      {(site.name)}}
              }

              p { "Welcome! " ( user.id)}


        })
        .into_response())
    } else {
        Ok(html::maud_page(html! {
            p { "Welcome! You need to login" }
            a href="/oidc/login" { "Login" }
        })
        .into_response())
    }
}

#[derive(Clone, Debug, Deserialize)]
struct PostNote {
    form_text: String,
}

pub struct UploadableFile {
    filename: String,
    contents: String,
}

// async fn post_note(
//     State(app_state): State<AppState>,
//     Form(form): Form<PostNote>,
// ) -> Result<Response, AppError> {
//     tracing::info!("Post form {:?}", form);
//     let text = form.form_text.replace("\r\n", "\n");
//     let uf = render_note(&app_state.templates, text);
//     &app_state.backend.write_file(&uf).await?;
//     Ok(Redirect::to("/").into_response())
// }
//
// fn render_note(t: &tera::Tera, form_text: String) -> UploadableFile {
//     let mut context = tera::Context::new();
//     context.insert("contents", &form_text);
//     context.insert("uuid", &uuid::Uuid::new_v4().to_string());
//
//     for name in t.get_template_names() {
//         tracing::info!("Template: {:?}", name);
//     }
//     let path = t.render("note.filename", &context);
//     let body = t.render("note.body", &context);
//     UploadableFile {
//         filename: path.expect("could not render"),
//         contents: body.expect("could not render"),
//     }
// }

// #[derive(Clone, Debug, Deserialize)]
// struct PostReply {
//     in_reply_to: String,
//     form_text: String,
// }
//
// async fn post_reply(
//     State(app_state): State<AppState>,
//     Form(form): Form<PostReply>,
// ) -> Result<Response, AppError> {
//     tracing::info!("Post form {:?}", form);
//     let text = form.form_text.replace("\r\n", "\n");
//     let uf = render_reply(&app_state.templates, form.in_reply_to, text);
//     &app_state.backend.write_file(&uf).await?;
//     // ...
//     Ok(Redirect::to("/").into_response())
// }
//
// fn render_reply(t: &tera::Tera, in_reply_to: String, form_text: String) -> UploadableFile {
//     let mut context = tera::Context::new();
//     context.insert("contents", &form_text);
//     context.insert("in_reply_to", &in_reply_to);
//     context.insert("uuid", &uuid::Uuid::new_v4().to_string());
//
//     for name in t.get_template_names() {
//         tracing::info!("Template: {:?}", name);
//     }
//     let path = t.render("reply.filename", &context);
//     let body = t.render("reply.body", &context);
//     UploadableFile {
//         filename: path.expect("could not render"),
//         contents: body.expect("could not render"),
//     }
// }
//
// #[derive(Clone, Debug, Deserialize)]
// struct PostLike {
//     in_like_of: String,
//     form_text: String,
// }
//
// async fn post_like(
//     State(app_state): State<AppState>,
//     Form(form): Form<PostLike>,
// ) -> Result<Response, AppError> {
//     tracing::info!("Post form {:?}", form);
//     let text = form.form_text.replace("\r\n", "\n");
//     let uf = render_like(&app_state.templates, form.in_like_of, text);
//     &app_state.backend.write_file(&uf).await?;
//     Ok(Redirect::to("/").into_response())
// }
//
// fn render_like(t: &tera::Tera, in_reply_to: String, form_text: String) -> UploadableFile {
//     let mut context = tera::Context::new();
//     context.insert("contents", &form_text);
//     context.insert("in_like_of", &in_reply_to);
//     context.insert("uuid", &uuid::Uuid::new_v4().to_string());
//
//     for name in t.get_template_names() {
//         tracing::info!("Template: {:?}", name);
//     }
//     let path = t.render("like.filename", &context);
//     let body = t.render("like.body", &context);
//     UploadableFile {
//         filename: path.expect("could not render"),
//         contents: body.expect("could not render"),
//     }
// }

// Make our own error that wraps `anyhow::Error`.
#[derive(Debug)]
pub struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("HTTP Error {:?}", &self);
        let desc = format!("Something went wrong: {}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            html::maud_page(html! {
            p {
               (desc)
               a href="/" {"Go home"}
            }}),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
