use serde::{Deserialize, Serialize};

use super::Tags;

pub type Pid = String;

pub type PidSet = super::Map<Pid, ProcessInfo>;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DeploymentStatus {
    /// during session creation
    #[serde(rename = "pending")]
    PENDING,
    /// after session creation, czysta
    #[serde(rename = "created")]
    CREATED,
    /// with at least one active child
    #[serde(rename = "running")]
    RUNNING,
    /// DIRTY: when no child is running, but some commands were already executed
    #[serde(rename = "configured")]
    CONFIGURED,
    /// during session removal
    #[serde(rename = "destroying")]
    DESTROYING,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentInfo {
    pub id: String,
    pub name: String,
    pub status: DeploymentStatus,
    #[serde(skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub processes: PidSet,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub tags: Tags,
}

impl From<gu_net::rpc::peer::PeerSessionInfo> for DeploymentInfo {
    fn from(peer: gu_net::rpc::peer::PeerSessionInfo) -> Self {
        DeploymentInfo {
            id: peer.id,
            name: peer.name,
            status: peer.status.into(),
            tags: peer.tags.into_iter().collect(),
            note: peer.note,
            processes: PidSet::new(),
        }
    }
}

impl From<gu_net::rpc::peer::PeerSessionStatus> for DeploymentStatus {
    fn from(raw_status: gu_net::rpc::peer::PeerSessionStatus) -> Self {
        use gu_net::rpc::peer::PeerSessionStatus;

        match raw_status {
            PeerSessionStatus::CREATED => DeploymentStatus::CREATED,
            PeerSessionStatus::CONFIGURED => DeploymentStatus::CONFIGURED,
            PeerSessionStatus::PENDING => DeploymentStatus::PENDING,
            PeerSessionStatus::DESTROYING => DeploymentStatus::DESTROYING,
            PeerSessionStatus::RUNNING => DeploymentStatus::RUNNING,
        }
    }
}
