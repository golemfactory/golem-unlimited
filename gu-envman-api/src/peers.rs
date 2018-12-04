
use gu_net::NodeId;
use std::collections::{BTreeSet, BTreeMap};
use std::cmp::Ordering;

pub type Tags = BTreeSet<String>;

pub type Pid = String;

pub type PidSet = BTreeMap<Pid, ProcessInfo>;


#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub node_id : NodeId,
    pub node_name : Option<String>,
    pub peer_addr : String,
    pub tags : Tags
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerDetails {
    pub node_id : NodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_name : Option<String>,
    pub peer_addr : String,
    #[serde(skip_serializing_if = "Tags::is_empty")]
    pub tags : Tags,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sessions : Vec<DeploymentInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DeploymentStatus {
    /// during session creation
    #[serde(rename="pending")]
    PENDING,
    /// after session creation, czysta
    #[serde(rename="created")]
    CREATED,
    /// with at least one active child
    #[serde(rename="running")]
    RUNNING,
    /// DIRTY: when no child is running, but some commands were already executed
    #[serde(rename="configured")]
    CONFIGURED,
    /// during session removal
    #[serde(rename="destroying")]
    DESTROYING,
}


#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentInfo {
    pub id: String,
    pub name: String,
    pub status: DeploymentStatus,
    pub tags: Vec<String>,
    pub note: Option<String>,
    pub processes: PidSet,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub tags: Vec<String>,
}

