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

use redacted::FullyRedacted;
use tower_cookies::CookieManagerLayer;

mod html;

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
struct Template {
    path: String,
    body: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum GithubConfig {
    PersonalTokenConfig(GithubTokenConfig),
    AppConfig(GithubAppConfig),
}
use octocrab::models::{InstallationRepositories, InstallationToken};
use octocrab::params::apps::CreateInstallationAccessToken;
use url::Url;
impl GithubConfig {
    async fn build_octocrab(&self) -> anyhow::Result<Octocrab> {
        match self {
            GithubConfig::PersonalTokenConfig(ptc) => Ok(Octocrab::builder()
                .personal_token(ptc.token.clone())
                .build()?),
            GithubConfig::AppConfig(ac) => {
                let k = jsonwebtoken::EncodingKey::from_rsa_pem(ac.app_key.as_bytes())?;
                let o = Octocrab::builder().app(ac.app_id.into(), k).build()?;

                let installations = o.apps().installations().send().await?.take_items();

                let mut create_access_token = CreateInstallationAccessToken::default();
                create_access_token.repositories = vec!["philipcristiano.com".to_string()];

                // By design, tokens are not forwarded to urls that contain an authority. This means we need to
                // extract the path from the url and use it to make the request.
                let access_token_url =
                    Url::parse(installations[0].access_tokens_url.as_ref().unwrap())?;

                let access: InstallationToken = o
                    .post(access_token_url.path(), Some(&create_access_token))
                    .await?;

                let octocrab = octocrab::OctocrabBuilder::new()
                    .personal_token(access.token)
                    .build();
                Ok(octocrab?)
            }
        }
    }

    fn repository(&self) -> anyhow::Result<String> {
        match self {
            GithubConfig::PersonalTokenConfig(ptc) => Ok(ptc.repository.clone()),
            GithubConfig::AppConfig(ac) => Ok("philipcristiano.com".to_string()),
        }
    }

    fn owner(&self) -> anyhow::Result<String> {
        match self {
            GithubConfig::PersonalTokenConfig(ptc) => Ok(ptc.owner.clone()),
            GithubConfig::AppConfig(ac) => Ok("philipcristiano".to_string()),
        }
    }
    fn branch(&self) -> anyhow::Result<String> {
        match self {
            GithubConfig::PersonalTokenConfig(ptc) => Ok(ptc.branch.clone()),
            GithubConfig::AppConfig(ac) => Ok("likes".to_string()),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct GithubTokenConfig {
    token: String,
    owner: String,
    repository: String,
    branch: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubAppConfig {
    app_id: u64,
    app_key: FullyRedacted<String>,
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

    let serve_assets = axum_embed::ServeEmbed::<StaticAssets>::new();
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        .route("/notes", post(post_note))
        .route("/replies", post(post_reply))
        .route("/likes", post(post_like))
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
    if let Some(user) = user {
        html::maud_page(html! {
              p { "Welcome! " ( user.id)}

              h2 {"Note"}
              form method="post" action="/notes" {
                textarea white-space="pre-wrap" id="form_text" class="border min-w-full" name="form_text" {}
                input type="submit" class="border" {}
              }

              h2 {"Reply"}
              form method="post" action="/replies" {
                input id="in_reply_to" class="border min-w-full" name="in_reply_to" {}
                textarea white-space="pre-wrap" id="form_text" class="border min-w-full" name="form_text" {}
                input type="submit" class="border" {}
              }
              h2 {"Like"}
              form method="post" action="/likes" {
                input id="in_like_of" class="border min-w-full" name="in_like_of" {}
                textarea white-space="pre-wrap" id="form_text" class="border min-w-full" name="form_text" {}
                input type="submit" class="border" {}
              }

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

async fn post_note(
    State(app_state): State<AppState>,
    Form(form): Form<PostNote>,
) -> Result<Response, AppError> {
    tracing::info!("Post form {:?}", form);
    let text = form.form_text.replace("\r\n", "\n");
    let uf = render_note(&app_state.templates, text);
    write_file(&app_state.github, &uf).await?;
    Ok(Redirect::to("/").into_response())
}

fn render_note(t: &tera::Tera, form_text: String) -> UploadableFile {
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

#[derive(Clone, Debug, Deserialize)]
struct PostReply {
    in_reply_to: String,
    form_text: String,
}

async fn post_reply(
    State(app_state): State<AppState>,
    Form(form): Form<PostReply>,
) -> Result<Response, AppError> {
    tracing::info!("Post form {:?}", form);
    let text = form.form_text.replace("\r\n", "\n");
    let uf = render_reply(&app_state.templates, form.in_reply_to, text);
    write_file(&app_state.github, &uf).await?;
    // ...
    Ok(Redirect::to("/").into_response())
}

fn render_reply(t: &tera::Tera, in_reply_to: String, form_text: String) -> UploadableFile {
    let mut context = tera::Context::new();
    context.insert("contents", &form_text);
    context.insert("in_reply_to", &in_reply_to);
    context.insert("uuid", &uuid::Uuid::new_v4().to_string());

    for name in t.get_template_names() {
        tracing::info!("Template: {:?}", name);
    }
    let path = t.render("reply.filename", &context);
    let body = t.render("reply.body", &context);
    UploadableFile {
        filename: path.expect("could not render"),
        contents: body.expect("could not render"),
    }
}

#[derive(Clone, Debug, Deserialize)]
struct PostLike {
    in_like_of: String,
    form_text: String,
}

async fn post_like(
    State(app_state): State<AppState>,
    Form(form): Form<PostLike>,
) -> Result<Response, AppError> {
    tracing::info!("Post form {:?}", form);
    let text = form.form_text.replace("\r\n", "\n");
    let uf = render_like(&app_state.templates, form.in_like_of, text);
    write_file(&app_state.github, &uf).await?;
    Ok(Redirect::to("/").into_response())
}

fn render_like(t: &tera::Tera, in_reply_to: String, form_text: String) -> UploadableFile {
    let mut context = tera::Context::new();
    context.insert("contents", &form_text);
    context.insert("in_like_of", &in_reply_to);
    context.insert("uuid", &uuid::Uuid::new_v4().to_string());

    for name in t.get_template_names() {
        tracing::info!("Template: {:?}", name);
    }
    let path = t.render("like.filename", &context);
    let body = t.render("like.body", &context);
    UploadableFile {
        filename: path.expect("could not render"),
        contents: body.expect("could not render"),
    }
}

use octocrab::models::repos::CommitAuthor;
use octocrab::Octocrab;
async fn write_file(github: &GithubConfig, uf: &UploadableFile) -> anyhow::Result<bool> {
    //let now = Local::now();
    //let id = uuid::Uuid::new_v4();
    //let filename = format!("content/notes/{}-{id}.md", now.format("%Y/%Y-%m-%dT%H:%M:%SZ"));
    //tracing::info!("Filename {:?}", filename);

    //let new_contents = format!("+++\n+++\n{contents}");

    let octocrab = github.build_octocrab().await?;
    octocrab
        .repos(github.owner()?, github.repository()?)
        .create_file(&uf.filename, "Create note", &uf.contents)
        .branch(github.branch()?)
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

// Make our own error that wraps `anyhow::Error`.
#[derive(Debug)]
struct AppError(anyhow::Error);

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
