use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CreateOptions {
    #[serde(default)]
    pub volumes: Vec<VolumeDef>,
    #[serde(default)]
    pub cmd: Option<Vec<String>>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<NetDef>,
}

impl CreateOptions {
    pub fn with_net(mut self, net: NetDef) -> Self {
        self.net = Some(net);
        self
    }
}

#[derive(Serialize, Deserialize, Hash, Clone, Eq, PartialEq)]
pub enum VolumeDef {
    BindRw { src: String, target: String },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum NetDef {
    #[serde(rename = "host")]
    Host {},
}

impl VolumeDef {
    pub fn source_dir(&self) -> Option<&String> {
        match self {
            VolumeDef::BindRw { src, target: _ } => Some(src),
        }
    }

    pub fn target_dir(&self) -> Option<&String> {
        match self {
            VolumeDef::BindRw { src: _, target } => Some(target),
        }
    }
}
