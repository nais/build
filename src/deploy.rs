use std::process::{ExitStatus, Stdio};
use thiserror::Error;

/// All field names corresponds with deploy client names
pub struct Config {
    pub apikey: String,
    pub cluster: String,
    pub deploy_server: String,
    pub environment: String,
    pub owner: String,
    /// Except `git_ref`, which is `--ref`
    pub git_ref: String,
    pub repository: String,
    pub resource: Vec<String>,
    pub var: Vec<String>,
    pub vars: String,
    pub wait: bool,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("deploy client exited with code {0}")]
    Deploy(ExitStatus),

    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

pub fn deploy(cfg: Config) -> Result<(), Error> {
    let mut process = std::process::Command::new("deploy");

    for resource_file in cfg.resource {
        process.arg("--resource").arg(resource_file);
    }
    for var in cfg.var {
        process.arg("--var").arg(var);
    }

    process
        .arg("--apikey").arg(cfg.apikey)
        .arg("--cluster").arg(cfg.cluster)
        .arg("--deploy-server").arg(cfg.deploy_server)
        .arg("--environment").arg(cfg.environment)
        .arg("--owner").arg(cfg.owner)
        .arg("--ref").arg(cfg.git_ref)
        .arg("--repository").arg(cfg.repository)
        .arg("--vars").arg(cfg.vars)
        .arg("--wait").arg(cfg.wait.to_string())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map(|exit_status| {
            if exit_status.success() {
                Ok(())
            } else {
                Err(Error::Deploy(exit_status))
            }
        })?
}
// Unused configuration options

//traceparent:               String,
// Actions                   bool
// DryRun                    bool
// GithubToken               string
// GrpcAuthentication        bool
// GrpcUseTLS                bool
// OpenTelemetryCollectorURL string
// PollInterval              time.Duration
// PrintPayload              bool
// Quiet                     bool
// Retry                     bool
// RetryInterval             time.Duration
// Team                      string
// Telemetry                 *telemetry.PipelineTimings
// TelemetryInput            string
// Timeout                   time.Duration
// TracingDashboardURL       string
