use serde_derive::*;
//use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HdEntryPoint {
    pub id: String,
    pub exec: String,
    #[serde(default)]
    pub args_prefix: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct HdVolume {}

#[derive(Serialize, Deserialize)]
pub enum Os {
    Linux,
    MacOs,
    Win64,
}

#[derive(Serialize, Deserialize)]
pub enum Arch {
    x86_64,
    Wasm32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HdManifest {
    pub id: String,
    pub name: String,
    pub author: Vec<String>,
    pub main: HdEntryPoint,
    pub entry_points: Vec<HdEntryPoint>,
    pub volumes: Vec<HdVolume>,
}
