use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Blob {
    path: Option<PathBuf>,
}

impl Blob {
    pub fn new() -> Blob {
        Blob {
            path: None,
        }
    }
}