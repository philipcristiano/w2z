use axum::{
    extract::{Query, State, FromRef},
    http::StatusCode,
    Form,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use clap::Parser;
use maud::{html, DOCTYPE};
use serde::Deserialize;
use std::fs;
use std::net::SocketAddr;

use tower_cookies::{Cookie, CookieManagerLayer, Cookies, Key};


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
struct GithubConfig {
    token: String,
    owner: String,
    repository: String,
    branch: String,
}
#[derive(FromRef, Clone, Debug, Deserialize)]
struct AppConfig {
    auth: service_conventions::oidc::AuthConfig,
    github: GithubConfig,
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

    let oidc_router = service_conventions::oidc::router(app_config.auth.clone());
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        .route("/", post(post_note))
        .nest("/oidc", oidc_router.with_state(app_config.auth.clone()))
        .with_state(app_config.clone())
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
// basic handler that responds with a static string
async fn root(user: Option<service_conventions::oidc::OIDCUser>) -> Response {
    if let Some(user) = user {
        html! {
         (DOCTYPE)
              p { "Welcome! " ( user.id)}
              @if let Some(name) = user.name {
                  p{ ( name ) }
              }
              @if let Some(email) = user.email {
                  p{ ( email ) }
              }

              form method="post" action="/" {
                textarea id="form_text" name="form_text" {}
                input type="submit" {}
              }

              a href="/oidc/login" { "Login" }
        }
        .into_response()
    } else {

        html! {
         (DOCTYPE)
            p { "Welcome! You need to login" }
            a href="/oidc/login" { "Login" }
        }.into_response()
    }
}

#[derive(Clone, Debug, Deserialize)]
struct PostNote {
    form_text: String,
}

async fn post_note(State(app_config): State<AppConfig>, Form(form): Form<PostNote>, ) -> Response {
    tracing::info!("Post form {:?}", form);
    write_file(&app_config.github, form.form_text).await;
    // ...
    Redirect::to("/").into_response()
}

use octocrab::models::repos::CommitAuthor;
use octocrab::Octocrab;
use chrono::Local;
async fn write_file(github: &GithubConfig, contents: String) -> anyhow::Result<bool> {

    let now = Local::now();
    let id = uuid::Uuid::new_v4();
    let filename = format!("content/notes/{}-{id}.md", now.format("%Y/%Y-%m-%dT%H:%M:%SZ"));
    tracing::info!("Filename {:?}", filename);

    let new_contents = format!("+++\n+++\n{contents}");

    let octocrab = Octocrab::builder().personal_token(github.token.clone()).build()?;
    octocrab.repos(&github.owner, &github.repository)
    .create_file(
        filename,
        "Create note ",
        &new_contents
    )
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
