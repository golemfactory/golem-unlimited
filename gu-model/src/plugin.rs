use semver::Version;
use serde::{Deserialize, Serialize};

/** Spec for plugin manifests **/
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub struct PluginManifest {
    pub id: String,
    pub version: Option<Version>,
    pub creator: Vec<String>,
    pub platform: Option<Platform>,
    pub provider: Option<ProviderActivator>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ProviderActivator {
    /// minimal supported provider version
    pub min_version: Option<Version>,
    pub simple_exec_env: Vec<SimpleExecEnvSpec>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub struct SimpleExecEnvSpec {
    pub code: String,
    pub exec: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    Universal, // javascript
    X86_64LinuxGnu,
    X86_64Windows,
    X86_64Darwin,
}

impl Platform {
    #[cfg(target_arch = "x86_64")]
    #[cfg(target_os = "linux")]
    fn current() -> Platform {
        Platform::X86_64LinuxGnu
    }

    #[cfg(target_arch = "x86_64")]
    #[cfg(target_os = "macos")]
    fn current() -> Platform {
        Platform::X86_64LinuxGnu
    }

    #[cfg(target_arch = "x86_64")]
    #[cfg(target_os = "windows")]
    fn current() -> Platform {
        Platform::X86_64LinuxGnu
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolveResult {
    ResolvedPath(String),
}

#[cfg(test)]
mod test {
    use crate::plugin::Platform;

    #[test]
    fn test_to_platform() {
        eprintln!("{}", serde_json::to_string(&Platform::current()).unwrap())
    }

}
