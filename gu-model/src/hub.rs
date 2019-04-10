use super::chrono::{DateTime, Utc};
use super::{Map, Version};
use gu_net::NodeId;
use serde_derive::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub ts: DateTime<Utc>,
    pub target: String,
    pub commit_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    #[serde(rename = "v")]
    pub version: Version,
    #[serde(flatten)]
    pub props: Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HubInfo {
    pub node_id: NodeId,
    pub version: Version,
    pub build: BuildInfo,
    #[serde(default)]
    pub caps: Map<String, Capability>,
}

#[cfg(test)]
mod test {
    use super::Map;
    use crate::hub::Capability;
    use crate::{BuildInfo, HubInfo};
    use actix_web::error::ParseError::Version;
    use serde_json;

    #[test]
    fn test_serialize() {
        let hub_info = HubInfo {
            node_id: "0xf6140a03926b0801cd891d2d128ebd8dffbda252"
                .parse()
                .unwrap(),
            version: "0.2.0".parse().unwrap(),
            build: BuildInfo {
                ts: "2019-03-06T15:12:31.221524706+00:00".parse().unwrap(),
                target: "x86_64-unknown-linux-gnu".to_string(),
                commit_hash: "57950ae41130b45e3a5aa00e65a50bea004928d1".to_string(),
            },
            caps: (vec![
                (
                    "gu.env.hd".to_string(),
                    Capability {
                        version: "0.1.0".parse().unwrap(),
                        props: Map::default(),
                    },
                ),
                (
                    "gu.env.docker".to_string(),
                    Capability {
                        version: "0.1.0".parse().unwrap(),
                        props: Map::default(),
                    },
                ),
                (
                    "gu.session.config".to_string(),
                    Capability {
                        version: "0.1.0".parse().unwrap(),
                        props: Map::default(),
                    },
                ),
                (
                    "gu.session.blob".to_string(),
                    Capability {
                        version: "0.2.0".parse().unwrap(),
                        props: Map::default(),
                    },
                ),
            ])
            .into_iter()
            .collect(),
        };

        eprintln!("{}", serde_json::to_string_pretty(&hub_info).unwrap());
    }

}
