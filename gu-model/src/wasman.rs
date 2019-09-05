use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CreateOptions {
    #[serde(default)]
    pub volumes: Vec<VolumeDef>,
    #[serde(default)]
    pub cmd: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Hash, Clone, Eq, PartialEq)]
pub enum VolumeDef {
    Rw { src: String, target: String },
    Ro { src: String, target: String },
    Tmp { target: String },
    Wo { target: String, src: String },
}
