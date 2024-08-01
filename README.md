# NAIS Build

NAIS Build is a continuous integration pipeline runner with _no external dependencies_.

You can run NAIS Build on your computer and get exactly the same results you would in another CI environment,
such as Github workflows or Jenkins.

* Builds, tests and lints your software using your specified SDK.
* Automatically packages your program into a Docker container with the latest security patches.
* Publishes built artifacts to Google Artifact Registry and GitHub releases.
* Deploys artifacts to Kubernetes or CDN.

## What changes between SDKs?
* Docker base image
* 

## How to build apps

### Go
* override go.mod with any sdk-controlled version bumps
  * "go 1.22"
  * "toolchain 1.22.5"
* go get
* go test
* linting
* staticcheck
* detect which binaries to build (cmd/*/*.go)
    * go build (flags for docker, architecture, etc)

### Rust
* cargo build --release