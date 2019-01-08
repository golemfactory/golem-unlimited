use super::deployment::DeploymentInfo;
use super::Tags;
use gu_net::NodeId;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub node_name: Option<String>,
    pub peer_addr: String,
    pub tags: Tags,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeerDetails {
    pub node_id: NodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_name: Option<String>,
    pub peer_addr: String,
    #[serde(skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<DeploymentInfo>,
}
