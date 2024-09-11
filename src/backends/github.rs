use crate::templating;

use crate::UploadableFile;
use octocrab::models::repos::CommitAuthor;
use octocrab::models::{InstallationRepositories, InstallationToken};
use octocrab::Octocrab;
use redacted::FullyRedacted;
use serde::Deserialize;

use http;

#[derive(Clone, Debug, Deserialize)]
pub struct GithubBackend {
    config: GithubConfig,
}

impl GithubBackend {
    pub fn from_config(config: GithubConfig) -> Self {
        GithubBackend { config }
    }

    pub async fn sites(&self) -> anyhow::Result<Vec<GithubSite>> {
        let o = self.config.build_octocrab().await?;
        let mut v = vec![];

        let installed_repos: InstallationRepositories = o
            .get("/installation/repositories", None::<&()>)
            .await
            .unwrap();
        for repo in installed_repos.repositories {
            tracing::debug!(installation= ?repo, "repo");
            if let Some(owner) = repo.owner {
                let n = format!("{}/{}", owner.login, repo.name);

                v.push(GithubSite {
                    name: n,
                    owner: owner.login,
                    repo: repo.name,
                })
            }
        }

        Ok(v)
    }

    pub async fn get_site_config(&self, site: &GithubSite) -> anyhow::Result<SiteConfig> {
        let octocrab = self.config.build_octocrab().await?;
        let mut content = octocrab
            .repos(&site.owner, &site.repo)
            .get_content()
            .path("w2z.toml")
            .r#ref("main")
            .send()
            .await?;
        let contents = content.take_items();
        let c = &contents[0];
        if let Some(decoded_content) = c.decoded_content() {
            Ok(toml::from_str(&decoded_content)?)
        } else {
            Err(anyhow::anyhow!("Couldn't do it!"))
        }
    }

    pub async fn write_file(self, site: GithubSite, uf: &UploadableFile) -> anyhow::Result<bool> {
        //let now = Local::now();
        //let id = uuid::Uuid::new_v4();
        //let filename = format!("content/notes/{}-{id}.md", now.format("%Y/%Y-%m-%dT%H:%M:%SZ"));
        //tracing::info!("Filename {:?}", filename);

        //let new_contents = format!("+++\n+++\n{contents}");

        let octocrab = self.config.build_octocrab().await?;
        octocrab
            .repos(site.owner, site.repo)
            .create_file(&uf.filename, "Create note", &uf.contents)
            .branch(self.config.branch()?)
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
}

#[derive(Clone, Debug, Deserialize)]
pub struct GithubConfig {
    app_id: u64,
    app_key: FullyRedacted<String>,

    #[serde(default = "default_branch")]
    branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}

use octocrab::params::apps::CreateInstallationAccessToken;
use url::Url;
impl GithubConfig {
    async fn build_octocrab(&self) -> anyhow::Result<Octocrab> {
        let o = self.build_app_octocrab().await?;

        let installations = o.apps().installations().send().await?.take_items();

        let create_access_token = CreateInstallationAccessToken::default();

        // By design, tokens are not forwarded to urls that contain an authority. This means we need to
        // extract the path from the url and use it to make the request.
        let access_token_url = Url::parse(installations[0].access_tokens_url.as_ref().unwrap())?;

        let access: InstallationToken = o
            .post(access_token_url.path(), Some(&create_access_token))
            .await?;

        let octocrab = octocrab::OctocrabBuilder::new()
            .personal_token(access.token)
            .build();
        Ok(octocrab?)
    }

    async fn build_app_octocrab(&self) -> anyhow::Result<Octocrab> {
        let k = jsonwebtoken::EncodingKey::from_rsa_pem(self.app_key.as_bytes())?;
        Ok(Octocrab::builder().app(self.app_id.into(), k).build()?)
    }

    fn repository(&self) -> anyhow::Result<String> {
        Ok("philipcristiano.com".to_string())
    }

    fn owner(&self) -> anyhow::Result<String> {
        Ok("philipcristiano".to_string())
    }
    fn branch(&self) -> anyhow::Result<String> {
        Ok(self.branch.clone())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct GithubSite {
    pub name: String,
    owner: String,
    repo: String,
}

impl From<SiteParams> for GithubSite {
    fn from(item: SiteParams) -> Self {
        let n = format!("{}/{}", item.owner, item.repo);

        GithubSite {
            name: n,
            owner: item.owner,
            repo: item.repo,
        }
    }
}

use axum::{
    extract::{FromRef, Path, RawForm, State},
    response::{IntoResponse, Redirect, Response},
    Form, Router,
};

#[derive(Clone, Debug, Deserialize)]
pub struct SiteParams {
    owner: String,
    repo: String,
}
use crate::{AppError, AppState};
pub async fn axum_get_site(
    State(app_state): State<AppState>,
    Path(site_params): Path<SiteParams>,
    user: Option<service_conventions::oidc::OIDCUser>,
) -> Result<Response, AppError> {
    let site: GithubSite = site_params.into();
    let config = &app_state.backend.get_site_config(&site).await?;
    let path_pref = format!("/b/github/{}", &site.name);
    let field_prefix = "fields".to_string();

    let body = crate::html::maud_page(maud::html! {
        @for template in config.templates.clone().into_iter() {
            p {
              h2 script="on click toggle .hidden on next <div/>" {(template.0)}
              div class="hidden" {
                form method="post" action={(&path_pref) "/new/" (template.0)} {
                  @for input_field in &template.1.input_fields {
                      (input_field.form_markup(&field_prefix, templating::FormLabel::Yes))
                      br {}
                  }
                  input type="submit" class="border" {}
                }
              }
              @for msg in &template.1.config_messages() {
                  p {(msg)}

              }
            }
        }

    });
    Ok(body.into_response())
}

#[derive(Clone, Debug, Deserialize)]
pub struct SiteConfig {
    templates: indexmap::IndexMap<String, templating::Template>,
}

impl SiteConfig {
    fn get_template(self, name: String) -> Option<templating::Template> {
        self.templates
            .into_iter()
            .find(|t| t.0 == name)
            .map(|(_s, t)| t)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TemplatePostParams {
    owner: String,
    repo: String,
    template_name: String,
}

impl From<TemplatePostParams> for GithubSite {
    fn from(item: TemplatePostParams) -> Self {
        let n = format!("{}/{}", item.owner, item.repo);

        GithubSite {
            name: n,
            owner: item.owner,
            repo: item.repo,
        }
    }
}

use serde_qs::Config;
pub async fn axum_post_template(
    State(app_state): State<AppState>,
    Path(path_params): Path<TemplatePostParams>,
    RawForm(form): RawForm,
) -> Result<Response, AppError> {
    tracing::info!("Post form {:?}", form);

    let qs = Config::new(5, false);
    let site: GithubSite = path_params.clone().into();
    let config = &app_state.backend.get_site_config(&site).await?;
    let post_data: templating::Blob = qs.deserialize_bytes(&form)?;
    tracing::info!("Post form data{:?}", post_data);

    let maybe_template = config.to_owned().get_template(path_params.template_name);
    if let Some(template) = maybe_template {
        let file_contents = template.as_toml(post_data.clone())?;
        let file_path = template.rendered_path(post_data)?;
        let uf = crate::UploadableFile {
            filename: file_path,
            contents: file_contents,
        };
        let _ = &app_state.backend.write_file(site, &uf).await?;
        Ok(Redirect::to("/").into_response())
    } else {
        return Ok((http::status::StatusCode::NOT_FOUND, "").into_response());
    }

    //let uf = render_like(&app_state.templates, form.in_like_of, text);
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
