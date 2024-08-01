enum Version {
    Latest,
    Major(String),
    Exact(String),
}

enum SDK {
    Go(Version),
    Rust(Version),
    Java(Version),
}

struct Go {}

impl Go {
    fn detect(_path: &str) {}

    fn docker_build_command() -> String {
        todo!()
    }
}