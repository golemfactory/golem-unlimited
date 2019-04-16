use super::deployment::DeploymentInfo;
use super::Tags;
use gu_net::NodeId;
use serde_derive::*;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub node_id: NodeId,
    #[serde(default)]
    pub node_name: Option<String>,
    pub peer_addr: String,
    #[serde(default)]
    pub tags: Tags,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeerDetails {
    pub node_id: NodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub node_name: Option<String>,
    pub peer_addr: String,
    #[serde(skip_serializing_if = "Tags::is_empty")]
    #[serde(default)]
    pub tags: Tags,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub sessions: Vec<DeploymentInfo>,
}
