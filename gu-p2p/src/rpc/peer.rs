use super::super::NodeId;

#[derive(Serialize, Deserialize)]
pub struct PeerInfo {
    node_name: String,
    node_id: NodeId,
    sessions: Vec<PeerSessionInfo>,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub enum PeerSessionStatus {
    PENDING,
    CREATED,
    RUNNING,
    CONFIGURED,
    DESTROYING,
}

#[derive(Serialize, Deserialize)]
pub struct PeerSessionInfo {
    id: String,
    name: String,
    status: PeerSessionStatus,
    tags: Vec<String>,
    note: Option<String>,
}

pub enum UpdatePeer {
    Update(PeerInfo),
    Delete(NodeId),
}

pub struct PeerManager {}
