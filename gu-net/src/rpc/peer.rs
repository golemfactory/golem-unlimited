use super::super::NodeId;
use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};

// TODO or HashSet?
pub type Tags = BTreeSet<String>;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub node_name: String,
    pub peer_addr: Option<String>,
    pub node_id: NodeId,
    pub sessions: Vec<PeerSessionInfo>,
    pub tags: Tags,
}

pub enum State {
    PENDING,

    CREATED,

    RUNNING,

    DIRTY,

    DESTROYING,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PeerSessionStatus {
    /// during session creation
    PENDING,
    /// after session creation, czysta
    CREATED,
    /// with at least one active child
    RUNNING,
    /// DIRTY: when no child is running, but some commands were already executed
    CONFIGURED,
    /// during session removal
    DESTROYING,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeerSessionInfo {
    pub id: String,
    pub name: String,
    pub status: PeerSessionStatus,
    pub tags: Tags,
    pub note: Option<String>,
    pub processes: HashSet<String>,
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

pub struct GetPeer(pub NodeId);

impl Message for GetPeer {
    type Result = Option<PeerInfo>;
}

impl Handler<GetPeer> for PeerManager {
    type Result = MessageResult<GetPeer>;

    fn handle(
        &mut self,
        msg: GetPeer,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<GetPeer>>::Result {
        MessageResult(self.peers.get(&msg.0).cloned())
    }
}

// Tag management
pub struct AddTags {
    pub node: NodeId,
    pub tags: Tags,
}

impl Message for AddTags {
    type Result = Option<()>;
}

impl Handler<AddTags> for PeerManager {
    type Result = Option<()>;
    fn handle(&mut self, msg: AddTags, ctx: &mut Self::Context) -> Self::Result {
        let mut peer = self
            .peers
            .get_mut(&msg.node)?;
        let mut tags = &mut peer.tags;
        for tag in msg.tags {
            tags.insert(tag);
        }
        Some(())
    }
}

pub struct DeleteTags {
    pub node: NodeId,
    pub tags: Tags,
}

impl Message for DeleteTags {
    type Result = Option<()>;
}

impl Handler<DeleteTags> for PeerManager {
    type Result = Option<()>;
    fn handle(&mut self, msg: DeleteTags, ctx: &mut Self::Context) -> Self::Result {
        let mut peer = self
            .peers
            .get_mut(&msg.node)?;
        let mut tags = &mut peer.tags;
        for tag in msg.tags {
            tags.remove(&tag);
        }
        Some(())
    }
}
