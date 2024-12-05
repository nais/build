# NAIS Build

NAIS Build, or `nb`, is an opinionated continuous integration pipeline runner,
tailored for developers on the NAIS platform.

## Features
* Generates best-practice Dockerfiles for standardized Go and Gradle projects.
* Builds Docker images with correct repository, namespace, team and date-based tag.
* Publish built artifacts to Google Artifact Registry.
* Deploys the built image using Nais deploy.
* No Dockerfile needed, _nb_ will generate one for you.
* Build target detection with zero configuration.
* Uses the latest build and runtime environments.
* Run and debug the CI pipeline on your local computer.
* It's very fast.

## Roadmap
* Support for many kinds of Go, Rust, Java, and Kotlin projects.
* Build, test, lint, and auditing using best practices.
* SBOM generation and signature.
* Publish built artifacts also to GHCR and GitHub releases.
* Intermediate build step caching.
* Deploy artifacts to CDN.
* Deploy profiles.
* Matrix builds.

## Design goals
* Run fast, and anywhere.
* Require minimal boilerplate for new pipelines.
* Stupidly simple configuration syntax.
* Cover most use cases with zero or minimal configuration.
* Flexible enough to allow build customization.

## Replaces
- `Dockerfile`. If NAIS Build is able to successfully detect your build parameters,
  you don't need a Dockerfile in your repository.
- _Github workflow YAML files_. NAIS Build will perform all the steps found in `nais-build-push`.

## Usage
Install using:

    cargo install --path .

Run the build pipeline from your local machine:

    nb build

Show the Dockerfile that NAIS Build generates and uses to build your program:

    nb dockerfile

### Proposed future commands

Validate configuration.

    nb check

Generate a configuration file based on default values, for easy extension.

    nb default-config > build.toml

Run from a Github Workflow, set up `.github/workflows/nb.yml` file that runs:

    nb

## Developing
This project is written in stable Rust, with a recommended minimal version of 1.80.

### Github workflow templates
* https://github.com/navikt/sif-gha-workflows/tree/main/.github/workflows
* https://github.com/navikt/fp-gha-workflows/tree/main/.github/workflows