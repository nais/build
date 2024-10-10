use thiserror::Error;

#[derive(Error, Debug)]
pub enum DetectBuildTargetError {
    #[error("filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("can't find {1}: {0} ")]
    FileError(std::io::Error, String),

    #[error("target name is empty")]
    EmptyFilename,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("detect build target: {0}")]
    DetectBuildTargetError(#[from] DetectBuildTargetError),
}

/// SDK is anything that can produce artifacts
pub trait DockerFileBuilder {
    fn builder_docker_image(&self) -> String;
    fn runtime_docker_image(&self) -> String;
    fn detect_build_targets(&self) -> Result<Vec<String>, DetectBuildTargetError>;
    fn dockerfile(&self) -> Result<String, Error>;
    fn filesystem_path(&self) -> String;
}

/// Build Go projects.
pub mod golang {
    use log::debug;
    use super::DetectBuildTargetError;
    use super::DockerFileBuilder;
    use super::Error;

    pub struct Golang(Config);

    pub struct Config {
        pub filesystem_path: String,
        pub docker_builder_image: String,
        pub docker_runtime_image: String,

        #[allow(dead_code)]
        pub start_hook: Option<String>,
        #[allow(dead_code)]
        pub end_hook: Option<String>,
    }

    pub fn new(cfg: Config) -> Result<Option<Golang>, Error> {
        let Ok(file_stat) = std::fs::metadata(cfg.filesystem_path.to_owned() + "/go.mod") else {
            return Ok(None);
        };
        debug!("Detected `go.mod` in project root");
        if !file_stat.is_file() {
            return Ok(None);
        }

        Ok(Some(Golang(cfg)))
    }

    impl DockerFileBuilder for Golang {
        fn builder_docker_image(&self) -> String {
            self.0.docker_builder_image.clone()
        }

        fn runtime_docker_image(&self) -> String {
            self.0.docker_runtime_image.clone()
        }

        /// Return a list of binaries that can be built.
        fn detect_build_targets(&self) -> Result<Vec<String>, DetectBuildTargetError> {
            let targets_path = format!("{}/cmd", self.0.filesystem_path.to_owned());
            std::fs::read_dir(&targets_path)
                .map_err(|e| DetectBuildTargetError::FileError(e, targets_path.clone()))?
                .map(|dir_entry| {
                    Ok(dir_entry?
                        .file_name()
                        .to_str()
                        .ok_or(DetectBuildTargetError::EmptyFilename)?
                        .to_string())
                })
                .collect()
        }

        fn dockerfile(&self) -> Result<String, Error> {
            let targets = self.detect_build_targets()?;
            let builder_image = &self.builder_docker_image();
            let runtime_image = &self.runtime_docker_image();
            let binary_build_commands: String = targets
                .iter()
                .map(|item| {
                    format!(
                        "RUN go build -a -installsuffix cgo -o /build/{} ./cmd/{}",
                        item, item
                    )
                })
                .fold(String::new(), |acc, item| acc + "\n" + &item)
                .trim()
                .to_string();
            let binary_copy_commands: String = targets
                .iter()
                .map(|item| format!("COPY --from=builder /build/{} /app/{}", item, item))
                .fold(String::new(), |acc, item| acc + "\n" + &item)
                .trim()
                .to_string();
            let default_target = if targets.len() == 1 {
                format!(r#"CMD ["/app/{}"]"#, targets[0])
            } else {
                "# Default CMD omitted due to multiple targets specified".to_string()
            };

            Ok(format!(
                r#"
# Dockerfile generated by NAIS build (version) at (timestamp)

#
# Builder image
#
FROM {builder_image} AS builder
ENV GOOS=linux
ENV CGO_ENABLED=0
WORKDIR /src

# Copy go.mod and go.sum files into source directory
# so that dependencies can be downloaded before the source code.
# This is a cache optimization step (???)
COPY go.* /src/
RUN go mod download
COPY . /src

# Start hook is run before testing
#RUN ___start_hook

# Test all modules
RUN go test ./...

# Build all binaries found in ./cmd/*
{binary_build_commands}

# End hook is run after build
#RUN ___end_hook

#
# Runtime image
#
FROM {runtime_image}
WORKDIR /app
{binary_copy_commands}
{default_target}
"#,
            ))
        }

        fn filesystem_path(&self) -> String {
            self.0.filesystem_path.clone()
        }
    }
}

/// Build Java and Kotlin applications using Gradle.
pub mod gradle {
    use log::debug;
    use super::DetectBuildTargetError;
    use super::DockerFileBuilder;
    use super::Error;

    pub struct Gradle(Config);

    pub struct Config {
        pub filesystem_path: String,
        pub docker_builder_image: String,
        pub docker_runtime_image: String,
        pub settings_file: Option<String>,

        #[allow(dead_code)]
        pub start_hook: Option<String>,
        #[allow(dead_code)]
        pub end_hook: Option<String>,
    }

    pub fn new(cfg: Config) -> Result<Option<Gradle>, Error> {
        let Ok(file_stat) = std::fs::metadata(cfg.filesystem_path.to_owned() + "/gradlew") else {
            return Ok(None);
        };
        debug!("Detected `gradlew` in project root");
        if !file_stat.is_file() {
            return Ok(None);
        }

        Ok(Some(Gradle(cfg)))
    }

    impl DockerFileBuilder for Gradle {
        fn builder_docker_image(&self) -> String {
            self.0.docker_builder_image.clone()
        }

        fn runtime_docker_image(&self) -> String {
            self.0.docker_runtime_image.clone()
        }

        /// Return a list of binaries that can be built.
        fn detect_build_targets(&self) -> Result<Vec<String>, DetectBuildTargetError> {
             Ok(vec!["test".to_string(), "shadowJar".to_string()])
        }

        fn dockerfile(&self) -> Result<String, Error> {
            let targets = self.detect_build_targets()?;
            let builder_image = &self.builder_docker_image();
            let runtime_image = &self.runtime_docker_image();
            let binary_build_commands: String = targets
                .iter()
                .map(|target| {
                    match &self.0.settings_file {
                        None => format!("RUN ./gradlew {target}"),
                        Some(settings_file) => format!("RUN ./gradlew -settings-file {settings_file} {target}"),
                    }
                })
                .fold(String::new(), |acc, item| acc + "\n" + &item)
                .trim()
                .to_string();
            let binary_copy_commands: String = "COPY --from=builder /src/build/libs/app-all.jar /app/app.jar".to_string();

            Ok(format!(
                r#"
# Dockerfile generated by NAIS build (version) at (timestamp)

#
# Builder image
#
FROM {builder_image} AS builder

WORKDIR /src
COPY . /src

# Build all binaries found in /src/src/main/
{binary_build_commands}

# End hook is run after build
#RUN ___end_hook

#
# Runtime image
#
FROM {runtime_image}

# TODO: Find out what this opts really does, what is the default?
ENV JAVA_OPTS='-XX:MaxRAMPercentage=90'

{binary_copy_commands}

CMD ["java", "-jar", "/app/app.jar"]
"#,
            ))
        }

        fn filesystem_path(&self) -> String {
            self.0.filesystem_path.clone()
        }
    }
}
