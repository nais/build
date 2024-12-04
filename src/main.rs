/// NAIS Build
use crate::Error::*;
use clap::{Parser, Subcommand};
use thiserror::Error;
use log::{debug, error, info};
use sdk::SDK;
use crate::nais_yaml::NaisYaml;

mod config;
mod docker;
mod nais_yaml;
mod sdk;
mod deploy;
mod google;
mod git;

/// Naisly build, test, release and deploy your application.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Root of the source code tree.
    #[arg(default_value = ".")]
    source_directory: String,

    /// Override the resulting Docker image name and tag.
    /// Using this option together with `release` or `deploy` will omit the build step.
    #[arg(long)]
    docker_image_name: Option<String>,

    /// Path to the Nais build configuration file.
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
    Release,
    /// Deploy `nais.yaml` and the newly built Docker image to a Nais cluster.
    Deploy {
        #[arg(long)]
        cluster: String
    },
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("no compatible SDKs for this source directory")]
    SDKNotDetected,

    #[error("filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("configuration is incomplete")]
    ConfigIncomplete,

    #[error("configuration file: {0}")]
    ConfigParse(#[from] config::file::Error),

    #[error("configuration: {0}")]
    Config(#[from] config::runtime::Error),

    #[error("deploy: {0}")]
    Deploy(#[from] deploy::Error),

    #[error("docker tag could not be generated: {0}")]
    DockerTag(#[from] docker::tag::Error),

    #[error("docker error: {0}")]
    Docker(#[from] docker::Error),

    #[error("detect nais.yaml: {0}")]
    DetectNaisYaml(#[from] nais_yaml::Error),

    #[error("google: {0}")]
    Google(#[from] google::Error),

    #[error("build error: {0}")]
    SDKError(#[from] sdk::Error),
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

async fn release(registry: &str, docker_image_name: &str) -> Result<(), Error> {
    // TODO: auth to ghcr
    // FIXME: determine if the correct user is authed (@nais.io vs @tenant)

    let token = google::get_gar_auth_token().await?;

    docker::login(registry, &token)?;
    docker::push(docker_image_name)?;
    docker::logout(registry)?;

    Ok(())
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

    let sdk = init_sdk(&args.source_directory, &cfg_file)?;

    let mut docker_name_config = docker::name::Config {
        registry: cfg.release.params.registry.clone(),
        tag: docker::tag::generate(&args.source_directory)?,
        team: cfg.team.clone(),
        app: cfg.app.clone(),
    };
    if let Some(user_provided_tag) = &args.docker_image_name {
        docker_name_config.tag = user_provided_tag.clone();
        debug!("Docker tag overridden");
    }
    let docker_image_name = cfg.release.docker_name_builder(docker_name_config).to_string();

    match args.command {
        Commands::Dockerfile => {
            println!("{}\n", sdk.dockerfile()?);
            info!("Docker image tag: {}", docker_image_name);
        }
        Commands::Build => {
            docker::build(&sdk, &docker_image_name)?;
        }
        Commands::Release => {
            // Release implies build, unless docker tag is supplied
            if args.docker_image_name.is_none() {
                docker::build(&sdk, &docker_image_name)?;
            }
            release(&cfg.release.params.registry, &docker_image_name).await?;
        }
        Commands::Deploy { cluster } => {
            let short_sha = git::short_sha(&args.source_directory)?;
            let git_meta = git::metadata(&args.source_directory)?;

            // Deploy implies build and release, unless docker tag is supplied
            if args.docker_image_name.is_none() {
                docker::build(&sdk, &docker_image_name)?;
                release(&cfg.release.params.registry, &docker_image_name).await?;
            }

            // FIXME: this should probably be a builder of some sort to validate the actual config
            let mut cfg= deploy::Config::try_new_from_env().ok_or(ConfigIncomplete)?;
            cfg.cluster = cluster;
            cfg.owner = git_meta.owner;
            cfg.git_ref = short_sha.to_string();
            cfg.repository = git_meta.name;
            cfg.resource = vec![nais_yaml_path.to_string()];
            cfg.var = vec![format!("image={docker_image_name}")];

            deploy::deploy(cfg)?;
        }
    }

    Ok(())
}

fn init_sdk(
    filesystem_path: &str,
    cfg: &config::file::File,
) -> Result<Box<dyn SDK>, Error> {
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
