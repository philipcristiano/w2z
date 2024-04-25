use axum::{
    extract::{FromRef, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use clap::Parser;
use maud::{html, DOCTYPE};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;

use tower_cookies::{Cookie, CookieManagerLayer, Cookies, Key};

use rust_embed::RustEmbed;
#[derive(RustEmbed, Clone)]
#[folder = "static/"]
struct StaticAssets;


mod html;

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
struct Template {
    path: String,
    body: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubConfig {
    token: String,
    owner: String,
    repository: String,
    branch: String,
}
#[derive(Clone, Debug, Deserialize)]
struct AppConfig {
    auth: service_conventions::oidc::OIDCConfig,
    github: GithubConfig,
    templates: HashMap<String, Template>,
}

#[derive(FromRef, Clone, Debug)]
struct AppState {
    auth: service_conventions::oidc::AuthConfig,
    github: GithubConfig,
    templates: tera::Tera,
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
            github: item.github,
            templates: create_tera(&item.templates),
        }
    }
}
use tower_http::trace::{self, TraceLayer};
use tracing::Level;

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
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        .route("/", post(post_note))
        .route("/static/tailwind.css", get(http_get_tailwind_css))
        .nest("/oidc", oidc_router.with_state(app_state.auth.clone()))
        .with_state(app_state.clone())
        .layer(CookieManagerLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        );

    let addr: SocketAddr = args.bind_addr.parse().expect("Expected bind addr");
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn create_tera(templates: &HashMap<String, Template>) -> tera::Tera {
    let mut t = tera::Tera::default();
    let mut vec_ts: Vec<(String, String)> = Vec::new();

    for (template_id, template_config) in templates.iter() {
        let filename_name = format!("{}.filename", template_id);
        let body_name = format!("{}.body", template_id);
        tracing::debug!(
            "Adding template: {}, {}",
            filename_name,
            template_config.path
        );
        tracing::debug!("Adding template: {}, {}", body_name, template_config.body);
        vec_ts.push((filename_name, template_config.path.clone()));
        vec_ts.push((body_name, template_config.body.clone()));
    }
    t.add_raw_templates(vec_ts).expect("Parse templates");
    t
}
// basic handler that responds with a static string
async fn root(user: Option<service_conventions::oidc::OIDCUser>) -> Response {
    use maud::PreEscaped;
    if let Some(user) = user {
        html::maud_page(html! {
              p { "Welcome! " ( user.id)}
              @if let Some(name) = user.name {
                  p{ ( name ) }
              }
              @if let Some(email) = user.email {
                  p{ ( email ) }
              }

              form method="post" action="/" {
                div id="form_text" class="border min-w-full" name="form_text" {}
                input type="submit" class="border" {}
              }
              script src="/static/quill/editor.js" {}
        })
        .into_response()
    } else {
        html::maud_page(html! {
            p { "Welcome! You need to login" }
            a href="/oidc/login" { "Login" }
        })
        .into_response()
    }
}

#[derive(Clone, Debug, Deserialize)]
struct PostNote {
    form_text: String,
}

struct UploadableFile {
    filename: String,
    contents: String,
}

async fn post_note(State(app_state): State<AppState>, Form(form): Form<PostNote>) -> Response {
    tracing::info!("Post form {:?}", form);
    let uf = render_file(&app_state.templates, form.form_text);
    write_file(&app_state.github, &uf).await;
    // ...
    Redirect::to("/").into_response()
}

fn render_file(t: &tera::Tera, form_text: String) -> UploadableFile {
    let mut terad = tera::Tera::default();
    let mut context = tera::Context::new();
    context.insert("contents", &form_text);
    context.insert("uuid", &uuid::Uuid::new_v4().to_string());

    for name in t.get_template_names() {
        tracing::info!("Template: {:?}", name);
    }
    let path = t.render("note.filename", &context);
    let body = t.render("note.body", &context);
    UploadableFile {
        filename: path.expect("could not render"),
        contents: body.expect("could not render"),
    }
}

use chrono::Local;
use octocrab::models::repos::CommitAuthor;
use octocrab::Octocrab;
async fn write_file(github: &GithubConfig, uf: &UploadableFile) -> anyhow::Result<bool> {
    //let now = Local::now();
    //let id = uuid::Uuid::new_v4();
    //let filename = format!("content/notes/{}-{id}.md", now.format("%Y/%Y-%m-%dT%H:%M:%SZ"));
    //tracing::info!("Filename {:?}", filename);

    //let new_contents = format!("+++\n+++\n{contents}");

    let octocrab = Octocrab::builder()
        .personal_token(github.token.clone())
        .build()?;
    octocrab
        .repos(&github.owner, &github.repository)
        .create_file(&uf.filename, "Create note", &uf.contents)
        .branch(&github.branch)
        .commiter(CommitAuthor {
            name: "Octocat".to_string(),
            email: "octocat@github.com".to_string(),
            date: None,
        })
        .author(CommitAuthor {
            name: "Ferris".to_string(),
            email: "ferris@rust-lang.org".to_string(),
            date: None,
        })
        .send()
        .await?;
    Ok(true)
}

async fn http_get_tailwind_css() -> impl IntoResponse {
    let t = include_bytes!("../tailwind/tailwind.css");
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("Content-Type", "text/css".parse().unwrap());
    (headers, t)
}
