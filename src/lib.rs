use std::collections::HashSet;
use std::fs::File;

use minijinja::syntax::SyntaxConfig;
use minijinja::{context, Environment};
use serde::{Deserialize, Serialize};

mod cli;
mod error;

pub use cli::{print_completions, Args};
pub use error::{Error, Result};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct Repo {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct Change {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub url: String,
    pub branch: String,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct PR {
    pub pr: u64,
}

impl PR {
    pub async fn to_change(&self, owner: String, repo: String) -> Result<Change> {
        let mut octo = octocrab::Octocrab::builder();
        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            octo = octo.personal_token(token)
        }
        let pr = octo.build()?.pulls(owner, repo).get(self.pr).await?;
        // TODO if state == "closed", we should be able dismiss it
        let mut url = pr
            .head
            .repo
            .ok_or(Error::GithubParseError("Missing repo head".to_string()))?
            .ssh_url
            .ok_or(Error::GithubParseError("Missing repo html url".to_string()))?
            .to_string();
        if let Some(strip) = url.strip_suffix(".git") {
            url = strip.to_string();
        }
        let branch = pr.head.ref_field;
        let title = Some(pr.title.unwrap_or(branch.clone()));
        Ok(Change { title, url, branch })
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(untagged)]
pub enum Update {
    Change(Change),
    PR(PR),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct Fork {
    pub name: String,
    pub target: Repo,
    pub upstream: Repo,
    pub changes: Vec<Update>,
}

impl Fork {
    pub fn parse_github(&self) -> Result<(String, String)> {
        let re = regex::Regex::new(r"github.com[/:]([^/]+)/([^/]+)")?;
        let caps = re
            .captures(&self.upstream.url)
            .ok_or(Error::GithubParseError(self.upstream.url.clone()))?;
        Ok((caps[1].to_string(), caps[2].to_string()))
    }

    pub async fn get_prs(&mut self) -> Result<()> {
        let (owner, repo) = self.parse_github()?;
        for item in &mut self.changes {
            if let Update::PR(pr) = item {
                *item = Update::Change(pr.to_change(owner.clone(), repo.clone()).await?);
            }
        }
        Ok(())
    }

    pub fn fill(&mut self) {
        // When upstream branch is not provided, we can use target branch
        if self.upstream.branch.is_none() {
            if let Some(branch) = &self.target.branch {
                self.upstream.branch = Some(branch.clone());
            }
        }
        // when change title is not provided, we can use branch name
        for item in &mut self.changes {
            if let Update::Change(change) = item {
                if let Change {
                    title: None,
                    url: _,
                    branch,
                } = change
                {
                    change.title = Some(branch.to_string());
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Repo>,
    pub forks: Vec<Fork>,
}

impl Config {
    pub async fn update(&mut self) -> Result<()> {
        for fork in &mut self.forks {
            fork.get_prs().await?;
            fork.fill();
        }
        Ok(())
    }

    pub fn remotes(&self) -> String {
        let mut remotes = HashSet::new();
        for fork in &self.forks {
            remotes.insert(fork.target.url.clone());
            remotes.insert(fork.upstream.url.clone());
            for update in &fork.changes {
                if let Update::Change(change) = update {
                    remotes.insert(change.url.clone());
                }
            }
        }
        remotes.into_iter().collect::<Vec<String>>().join(" ")
    }

    pub fn generate(&mut self, args: &Args) -> Result<()> {
        // use a syntax which won't mess too much with bash for shellcheck
        let syntax = SyntaxConfig::builder()
            .block_delimiters("#{", "}#")
            .variable_delimiters("'{", "}'")
            .comment_delimiters("#/*", "#*/")
            .build()?;
        let mut env = Environment::new();
        env.set_syntax(syntax);
        env.add_filter("remote_name", remote_name);
        env.add_template("update.sh", include_str!("update.sh"))?;
        let tmpl = env.get_template("update.sh").unwrap();
        println!(
            "{}",
            tmpl.render(context! {
                config => self.config,
                forks => self.forks,
                remotes => self.remotes(),
                push => args.push,
            })
            .unwrap()
        );
        Ok(())
    }
}

pub fn remote_name(value: String) -> String {
    value
        .replace("https://", "")
        .replace("git@", "")
        .replace(":", "/")
}

pub struct ForkManager {
    args: Args,
    config: Config,
}

impl ForkManager {
    pub async fn new(args: Args) -> Result<Self> {
        let config_file = File::open(&args.config_file)?;
        let mut config: Config = serde_yml::from_reader(config_file)?;
        config.update().await?;
        Ok(Self { args, config })
    }

    pub async fn main(&mut self) -> Result<()> {
        if self.args.process()? {
            if self.args.dry_run {
                dbg!(&self.config);
            } else {
                self.config.generate(&self.args)?;
            }
        }
        Ok(())
    }
}
