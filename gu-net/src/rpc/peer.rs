use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;

use super::super::NodeId;
use gu_persist::config::ConfigModule;
use std::io;
use std::path::PathBuf;
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
    path: PathBuf,
    saved_tags: HashMap<NodeId, Tags>,
}

impl PeerManager {
    fn get_tags_mut<'a>(&'a mut self, node_id: &NodeId) -> Option<&'a mut Tags> {
        //let peers = &mut self.peers;
        let saved_tags = &mut self.saved_tags;
        let tags = self.peers.get_mut(node_id).map(|peer| &mut peer.tags);
        tags.or_else(move || saved_tags.get_mut(node_id))
    }
}

impl Actor for PeerManager {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut <Self as Actor>::Context) {
        self.path = ConfigModule::new().work_dir().join("tags");
        // FIXME this one is awful
        let tags_serialized = fs::read_to_string(&self.path).unwrap_or_else(|e| match e.kind() {
            io::ErrorKind::NotFound => "{}".into(),
            _ => panic!(
                "Error reading saved tags from {}: {:?}",
                self.path.to_string_lossy(),
                e
            ),
        });
        let tags: HashMap<NodeId, Tags> = serde_json::from_str(&tags_serialized)
            .unwrap_or_else(|e| panic!("Deserialization of saved tags failed: {}", e));
        self.saved_tags = tags;
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        let tags_new = self
            .peers
            .iter()
            .map(|(node, info)| (*node, info.tags.clone())); // TODO can we avoid the clone?
        self.saved_tags.extend(tags_new);

        let tags_serialized =
            serde_json::to_string(&self.saved_tags).expect("Serialization of tags failed");
        fs::write(&self.path, tags_serialized).unwrap_or_else(|e| {
            error!(
                "Error saving tags to {}: {}",
                self.path.to_string_lossy(),
                e
            )
        });
    }
}

impl Default for PeerManager {
    fn default() -> Self {
        PeerManager {
            peers: HashMap::new(),
            path: PathBuf::new(),
            saved_tags: Default::default(),
        }
    }
}

impl Supervised for PeerManager {}

impl SystemService for PeerManager {}

impl Handler<UpdatePeer> for PeerManager {
    type Result = ();

    fn handle(&mut self, msg: UpdatePeer, ctx: &mut Self::Context) {
        match msg {
            UpdatePeer::Update(mut info) => {
                if let Some(tags) = self.saved_tags.get(&info.node_id) {
                    info.tags = tags.clone(); // TODO can we avoid the clone?
                }
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
    type Result = Result<(), ()>;
}

impl Handler<AddTags> for PeerManager {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: AddTags, ctx: &mut Self::Context) -> Self::Result {
        // If the peer is connected, then we should modify self.peers, as it will be
        // written at the actor stop.
        // Otherwise, we need to change self.saved_tags
        let mut tags = self.get_tags_mut(&msg.node).ok_or(())?;
        tags.extend(msg.tags);
        Ok(())
    }
}

pub struct DeleteTags {
    pub node: NodeId,
    pub tags: Tags,
}

impl Message for DeleteTags {
    type Result = Result<(), ()>;
}

impl Handler<DeleteTags> for PeerManager {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: DeleteTags, ctx: &mut Self::Context) -> Self::Result {
        let mut tags = self.get_tags_mut(&msg.node).ok_or(())?;
        for tag in msg.tags {
            tags.remove(&tag);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::prelude::*;
    #[test]
    fn test_tags() {
        let node = NodeId::default();
        let info = PeerInfo {
            node_name: "".into(),
            peer_addr: None,
            node_id: node,
            sessions: vec![],
            tags: Tags::default(),
        };

        let mut system = System::new("test");
        // TODO in this test the PeerManager is probably not stopped,
        // no tags are serialized afterwards
        let mgr = PeerManager::default().start();
        let fut = mgr
            .send(UpdatePeer::Update(info))
            .and_then(|()| mgr.send(GetPeer(node)))
            .and_then(|info| Ok(info.unwrap().tags))
            .and_then(|tags| {
                // FIXME this test will fail if there are some saved tags
                // in ~/.local/share/golemunlimited/tags
                // we probably need to mock the path
                assert_eq!(tags, Tags::default());
                Ok(())
            })
            .and_then(|()| {
                let mut new_tags = Tags::default();
                new_tags.insert("test".into());
                mgr.send(AddTags {
                    node: node,
                    tags: new_tags,
                })
            })
            .and_then(|ret| {
                ret.unwrap();
                System::current().stop();
                Ok(())
            });
        system.block_on(fut).unwrap();
    }
}
