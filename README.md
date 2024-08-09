# NAIS Build

Opinionated continuous integration pipeline runner,
tailored for NAV IT developers on the NAIS platform.

## Design goals
* Run fast, and anywhere.
* Require minimal boilerplate for new pipelines.
* Stupidly simple configuration syntax.
* Cover most use cases with zero or minimal configuration.
* Flexible enough to allow build customization.

## Features
* Build, test, lint, and audit your source code.
* Automatically uses the latest build and runtime environments.
* Packages your program into a Docker container.
* Publishes built artifacts to Google Artifact Registry and GitHub releases.
* Deploys artifacts to Kubernetes or your team's CDN bucket.
* Run and debug the pipeline on your local computer.
* No Dockerfile needed.
* Supports Go, Rust, Java, and Kotlin.
* Matrix builds.

## Replaces

- `Dockerfile`. If NAIS Build is able to successfully detect your build parameters,
  you don't need a Dockerfile in your repository.
- _Github workflow YAML files_. NAIS Build will perform all the steps found in `nais-build-sign-push`.

## Usage
Run the build pipeline from your local machine.

    nb

Run the build pipeline from your local machine, but use a pre-defined Dockerfile
instead of using auto-detected parameters. This is useful for complex builds.
NAIS Build assumes that you run tests and lint as part of this step.

    nb --dockerfile=Dockerfile

Generate a configuration file based on default values, for easy extension.

    nb default-config > build.toml

Validate configuration.

    nb check

Show the Dockerfile that NAIS Build generates and uses to build your program:

    nb dockerfile

Run from a Github Workflow, set up `.github/workflows/nb.yml` file that runs:

    nb

## Developing
This project is written in stable Rust, with a recommended minimal version of 1.80.

### Build process

#### Go
* override go.mod with any sdk-controlled version bumps
  * "go 1.22"
  * "toolchain 1.22.5"
* go get
* go test
* linting?
* staticcheck?
* detect which binaries to build (cmd/*/*.go)
    * go build (flags for docker, architecture, etc)

#### Rust
* cargo build --release

#### Java
* TODO

#### Kotlin
* TODO