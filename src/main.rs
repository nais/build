/// NAIS Build
use crate::Error::*;
use clap::{Parser, Subcommand};
use thiserror::Error;
use log::{debug, error, info};
use sdk::DockerFileBuilder;
use crate::nais_yaml::NaisYaml;

mod config;
mod docker;
mod nais_yaml;
mod sdk;

/// NAISly build, test, lint, check and deploy your application.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Root of the source code tree.
    #[arg(default_value = ".")]
    source_directory: String,

    /// Path to the NAIS build configuration file.
    // ... TODO: or nais.toml?
    #[arg(long)]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Detect build parameters, generate a Dockerfile for your project, and print it to standard output.
    Dockerfile,
    /// Build your project, resulting in a Docker image. Implies the `dockerfile` command.
    Build,
    /// Release this project's verified Docker image onto GAR or GHCR.
    Release {
        /// Use a tag from a Docker image that is already built and exists on the system.
        /// Omitting this flag implies the `build` command.
        #[arg(long)]
        tag: Option<String>,
    },
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("no compatible SDKs for this source directory")]
    SDKNotDetected,

    #[error("filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("configuration file: {0}")]
    ConfigParse(#[from] config::file::Error),

    #[error("configuration: {0}")]
    Config(#[from] config::runtime::Error),

    #[error("docker tag could not be generated: {0}")]
    DockerTag(#[from] docker::tag::Error),

    #[error("docker error: {0}")]
    Docker(#[from] docker::Error),

    #[error("detect nais.yaml: {0}")]
    DetectNaisYaml(#[from] nais_yaml::Error),

    #[error("build error: {0}")]
    SDKError(#[from] sdk::Error),

    #[error("google cloud auth error: {0}")]
    GoogleCloudAuthError(#[from] google_cloud_auth::error::Error),

    #[error("google cloud auth token error {0}")]
    GoogleCloudAuthTokenError(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// Read configuration file from disk and merge it with the
/// `default.toml` [built-in config](../default.toml).
///
/// If a configuration file name is not set explicitly, this function will
/// detect whether a config file with the default file name exists on disk.
/// If it does, it is used implicitly. If not, we ignore any read errors.
fn read_config(args: &Cli) -> Result<config::file::File, Error> {
    const DEFAULT_CONFIG_FILE: &str = "nb.toml";

    // Typically found in project root, e.g. ./nb.toml
    let config_path = format!("{}/{}", args.source_directory, DEFAULT_CONFIG_FILE);

    let config_file = match &args.config {
        None => {
            if std::fs::metadata(&config_path)
                .and_then(|metadata| Ok(metadata.is_file()))
                .unwrap_or(false)
            {
                Some(config_path.into())
            } else {
                None
            }
        }
        Some(c) => Some(c.clone()),
    };

    Ok(if let Some(config_file) = config_file {
        config::file::File::default_with_user_config_file(&config_file)?
    } else {
        config::file::File::default()
    })
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(_) => std::process::exit(0),
        Err(err) => {
            error!("fatal: {}", err.to_string());
            std::process::exit(1)
        }
    }
}

async fn run() -> Result<(), Error> {
    env_logger::init();

    let args = Cli::parse();
    let cfg_file = read_config(&args)?;

    info!("NAIS build 1.0.0");

    let nais_yaml_path = nais_yaml::detect_nais_yaml(&args.source_directory)?;
    info!("nais.yaml detected at {nais_yaml_path}");

    let nais_yaml_data = NaisYaml::parse_file(&nais_yaml_path)?;

    let cfg = config::runtime::Config::new(&cfg_file, nais_yaml_data).map_err(Config)?;

    info!("Application name detected: {}", &cfg.app);
    // FIXME: cfg.team might be an empty string
    info!("Team detected: {}", &cfg.team);

    let mut docker_name_config = docker::name::Config {
        registry: cfg.release.params.registry.clone(),
        tag: docker::tag::generate(&args.source_directory)?,
        team: cfg.team,
        app: cfg.app,
    };

    match args.command {
        Commands::Dockerfile => {
            let docker_image_name = cfg.release.docker_name_builder(docker_name_config).to_string();
            let sdk = init_sdk(&args.source_directory, &cfg_file)?;
            println!("{}\n", sdk.dockerfile()?);
            info!("Docker image tag: {}", docker_image_name);
            Ok(())
        }
        Commands::Build => {
            let docker_image_name = cfg.release.docker_name_builder(docker_name_config).to_string();
            let sdk = init_sdk(&args.source_directory, &cfg_file)?;
            docker::build(sdk, &docker_image_name)?;
            Ok(())
        }
        Commands::Release { tag } => {
            if let Some(tag) = &tag {
                docker_name_config.tag = tag.clone()
            }
            let docker_image_name = cfg.release.docker_name_builder(docker_name_config).to_string();
            info!("Docker image tag: {}", docker_image_name);

            if tag.is_none() {
                debug!("tag not supplied, building Docker image");
                let sdk = init_sdk(&args.source_directory, &cfg_file)?;
                docker::build(sdk, &docker_image_name)?;
            }

            // TODO: auth to ghcr
            let token = get_gar_auth_token().await?;
            let token = token.strip_prefix("Bearer ").unwrap_or(&token).to_string();

            docker::login(cfg.release.params.registry.clone(), token)?;
            docker::push(docker_image_name)?;
            docker::logout(cfg.release.params.registry.clone())?;
            Ok(())
        }
    }
}


async fn get_gar_auth_token() -> Result<String, Error> {
    use google_cloud_auth::{project::Config, token::DefaultTokenSourceProvider};
    use google_cloud_token::TokenSourceProvider as _;

    let audience = "https://oauth2.googleapis.com/token/";
    let scopes = [
        "https://www.googleapis.com/auth/cloud-platform",
    ];

    let config = Config::default()
        .with_audience(audience)
        .with_scopes(&scopes);
    let tsp = DefaultTokenSourceProvider::new(config).await.map_err(GoogleCloudAuthError)?;
    let ts = tsp.token_source();
    let token = ts.token().await.map_err(GoogleCloudAuthTokenError)?;
    Ok(token)
}

fn init_sdk(
    filesystem_path: &str,
    cfg: &config::file::File,
) -> Result<Box<dyn DockerFileBuilder>, Error> {
    let sdk = cfg.sdk.clone().unwrap();

    match sdk::golang::new(sdk::golang::Config {
        filesystem_path: filesystem_path.to_string(),
        docker_builder_image: sdk.go.build_docker_image.clone(),
        docker_runtime_image: sdk.go.runtime_docker_image.clone(),
        start_hook: None,
        end_hook: None,
    }) {
        Ok(Some(sdk)) => {
            return Ok(Box::new(sdk));
        }
        Ok(None) => {}
        Err(err) => return Err(Error::from(err)),
    }

    match sdk::gradle::new(sdk::gradle::Config {
        filesystem_path: filesystem_path.to_string(),
        docker_builder_image: sdk.gradle.build_docker_image.clone(),
        docker_runtime_image: sdk.gradle.runtime_docker_image.clone(),
        settings_file: sdk.gradle.settings_file.clone(),
        start_hook: None,
        end_hook: None,
    }) {
        Ok(Some(sdk)) => {
            return Ok(Box::new(sdk));
        }
        Ok(None) => {}
        Err(err) => return Err(Error::from(err)),
    }

    match sdk::maven::new(sdk::maven::Config {
        filesystem_path: filesystem_path.to_string(),
        docker_builder_image: sdk.maven.build_docker_image.clone(),
        docker_runtime_image: sdk.maven.runtime_docker_image.clone(),
        start_hook: None,
        end_hook: None,
    }) {
        Ok(Some(sdk)) => {
            return Ok(Box::new(sdk));
        }
        Ok(None) => {}
        Err(err) => return Err(Error::from(err)),
    }
    // try next sdk

    Err(SDKNotDetected)
}
