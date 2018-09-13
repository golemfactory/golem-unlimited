use super::super::NodeId;
use actix::prelude::*;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerInfo {
    pub node_name: String,
    pub peer_addr: Option<String>,
    pub node_id: NodeId,
    pub sessions: Vec<PeerSessionInfo>,
    pub tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PeerSessionStatus {
    PENDING,
    CREATED,
    RUNNING,
    CONFIGURED,
    DESTROYING,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerSessionInfo {
    pub id: String,
    pub name: String,
    pub status: PeerSessionStatus,
    pub tags: Vec<String>,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub enum UpdatePeer {
    Update(PeerInfo),
    Delete(NodeId),
}

impl Message for UpdatePeer {
    type Result = ();
}

pub struct PeerManager {
    peers: HashMap<NodeId, PeerInfo>,
}

impl Actor for PeerManager {
    type Context = Context<Self>;
}

impl Default for PeerManager {
    fn default() -> Self {
        PeerManager {
            peers: HashMap::new(),
        }
    }
}

impl Supervised for PeerManager {}

impl SystemService for PeerManager {}

impl Handler<UpdatePeer> for PeerManager {
    type Result = ();

    fn handle(&mut self, msg: UpdatePeer, ctx: &mut Self::Context) {
        match msg {
            UpdatePeer::Update(info) => {
                let _ = self.peers.insert(info.node_id, info);
            }
            UpdatePeer::Delete(node_id) => {
                let _ = self.peers.remove(&node_id);
            }
        }
    }
}

pub struct ListPeers;

impl Message for ListPeers {
    type Result = Vec<PeerInfo>;
}

impl Handler<ListPeers> for PeerManager {
    type Result = MessageResult<ListPeers>;

    fn handle(
        &mut self,
        msg: ListPeers,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<ListPeers>>::Result {
        MessageResult(self.peers.values().cloned().collect())
    }
}
