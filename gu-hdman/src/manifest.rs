use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HdEntryPoint {
    pub id: String,
    pub exec: String,
    #[serde(default)]
    pub args_prefix: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HdVolume {}

#[derive(Serialize, Deserialize, Debug)]
pub enum Os {
    Linux,
    MacOs,
    Win64,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_camel_case_types)]
pub enum Arch {
    x86_64,
    Wasm32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HdConfiguration {
    os: Os,
    cpu_arch: Arch,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HdManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: Vec<String>,
    pub main: HdEntryPoint,
    pub entry_points: Vec<HdEntryPoint>,
    #[serde(default)]
    pub volumes: Vec<HdVolume>,
    pub valid_configurations: Vec<HdConfiguration>,
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;

    #[test]
    fn test_gu_factor() {
        let b = include_bytes!("../test-data/gu-factor.json");

        let m: HdManifest = serde_json::from_slice(b.as_ref()).unwrap();

        assert_eq!(m.id, "unlimited.golem.network/examples/gu-factor/v0.1");
        assert_eq!(m.main.exec, "gu-factor");
        assert_eq!(m.main.args_prefix, Vec::<String>::new());
        eprintln!("manifest={:?}", m)
    }
}
