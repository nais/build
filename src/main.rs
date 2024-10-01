use clap::{Parser, Subcommand};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Check if the environment is ready
    Check {
        /// The environment to check
        #[arg(short, long, default_value = "development")]
        environment: String,
    },
    /// Build the Dockerfile
    Dockerfile,
    /// Build builds
    Build,
}

fn main() -> Result<(), Error> {
    let args = Cli::parse();

    match args.command {
        Commands::Check { environment } => {
            println!("hello {}", environment);
            Ok(())
        }
        Commands::Dockerfile => {
            let sdk = detect_sdk("/")?;
            let dockerfile = DockerBuildParams::from(sdk);
            println!("{}", dockerfile.dockerfile());
            Ok(())
        }
        Commands::Build => {
            Ok(())
        }
    }
}

#[allow(dead_code)]
struct SDK {
    language: (),
    version: (),
    build_image: String,
    runtime_image: String,
}

#[derive(Debug)]
enum Error {}

fn detect_sdk(_filesystem_path: &str) -> Result<SDK, Error> {
    Ok(SDK {
        language: (),
        version: (),
        build_image: "golang:1".into(),
        runtime_image: "golang:1".into(),
    })
}

struct DockerBuildParams {
    builder_image: String,
    runtime_image: String,
    #[allow(dead_code)]
    start_hook: Option<String>,
    #[allow(dead_code)]
    end_hook: Option<String>,
    binaries: Vec<String>,
}

impl DockerBuildParams {
    fn dockerfile(&self) -> String {
        let builder_image = &self.builder_image;
        let runtime_image = &self.runtime_image;
        let binary_commands: String = self.binaries.iter()
            .fold(String::new(), |acc, item| {
                format!("{}\nRUN cd cmd/{} && go build -a -installsuffix cgo -o {}", acc, item, item)
            });

        format!(r#"
FROM {builder_image} AS builder
#RUN ___start_hook
ENV GOOS=linux
ENV CGO_ENABLED=0
WORKDIR /src
COPY go.* /src/
RUN go mod download
COPY . /src
RUN go test ./...
{binary_commands}
#RUN ___end_hook

FROM {runtime_image}
WORKDIR /app
COPY --from=builder /src/cmd/naiserator/naiserator /app/naiserator
COPY --from=builder /src/cmd/naiserator_webhook/naiserator_webhook /app/naiserator_webhook
CMD ["/app/naiserator"]
"#,
        )

        // FROM ___sdk_build_docker_image AS builder
        // RUN ___start_hook
        // ENV GOOS=linux
        // ENV CGO_ENABLED=0
        // WORKDIR /src
        // COPY go.* /src/
        // RUN go mod download
        // COPY . /src
        // RUN make test
        // RUN cd cmd/naiserator && go build -a -installsuffix cgo -o naiserator
        // RUN cd cmd/naiserator_webhook && go build -a -installsuffix cgo -o naiserator_webhook
        // RUN ___end_hook
    }
}

impl From<SDK> for DockerBuildParams {
    fn from(sdk: SDK) -> Self {
        Self {
            builder_image: sdk.build_image,
            runtime_image: sdk.runtime_image,
            start_hook: None,
            end_hook: None,
            binaries: vec![
                "naiserator".into(),
                "naiserator_webhook".into(),
            ],
        }
    }
}