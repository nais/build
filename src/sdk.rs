pub enum Version {
    Latest,
    Major(usize),
    Exact(String),
}

pub enum SDK {
    Go,
    Rust,
    Java,
}