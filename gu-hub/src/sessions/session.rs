use actix_web::HttpResponse;
use bytes::Bytes;
use chrono::DateTime;
use chrono::Utc;
use futures::{future, prelude::*, stream};
use gu_base::files::{read_async, write_async};
use gu_model::session::{BlobInfo, Metadata};
use gu_net::{rpc::peer, NodeId};
use serde_json;
use sessions::{
    blob::Blob,
    responses::{SessionErr, SessionOk, SessionResult},
};
use std::{
    cmp,
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

pub struct Session {
    info: SessionInfo,
    state: Metadata,
    path: PathBuf,
    next_id: u64,
    storage: HashMap<u64, Blob>,
    version: u64,
    peers: HashMap<NodeId, PeerState>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SessionInfo {
    pub name: Option<String>,
    pub created: DateTime<Utc>,
}

impl Default for SessionInfo {
    fn default() -> Self {
        SessionInfo {
            name: None,
            created: Utc::now(),
        }
    }
}

#[derive(Default)]
struct PeerState {
    deployments: HashSet<String>,
}

pub(crate) fn entries_id_iter(path: &PathBuf) -> impl Iterator<Item = u64> {
    fs::read_dir(path)
        .expect("Cannot read session directory")
        .into_iter()
        .map(|entry| {
            entry
                .map_err(|err| error!("Invalid session file: {}", err))
                .and_then(|e| {
                    e.file_name()
                        .to_str()
                        .ok_or_else(|| error!("Invalid session filename"))
                        .and_then(|s| {
                            s.clone().parse::<u64>().map_err(|e| {
                                if !s.starts_with('.') {
                                    error!("Invalid session filename: {}", e)
                                }
                            })
                        })
                })
        })
        .filter(|res| res.is_ok())
        .map(|id| id.unwrap())
}

impl Session {
    pub fn new(
        info: SessionInfo,
        path: PathBuf,
    ) -> (Session, impl Future<Item = (), Error = SessionErr>) {
        let info_bytes = serde_json::to_string(&info)
            .map_err(|_| SessionErr::FileError("Invalid info file".to_string()))
            .and_then(|s| Ok(Bytes::from(s)));

        let session = Session {
            info,
            state: Metadata::default(),
            path: path.clone(),
            next_id: 0,
            storage: HashMap::new(),
            version: 0,
            peers: HashMap::new(),
        };

        let fut = fs::DirBuilder::new()
            .create(&path)
            .map_err(|e| SessionErr::DirectoryCreationError(e.to_string()))
            .into_future()
            .and_then(move |_| info_bytes)
            .and_then(move |info| {
                write_async(stream::once::<_, ()>(Ok(info)), path.join(".info"))
                    .map_err(|e| SessionErr::FileError(e))
            });

        (session, fut)
    }

    pub fn from_existing(path: PathBuf) -> impl Future<Item = Self, Error = String> {
        let info_fut = read_async(path.join(".info")).concat2().and_then(|a| {
            serde_json::from_slice::<SessionInfo>(a.as_ref()).map_err(|e| e.to_string())
        });

        let mut s = Session {
            info: SessionInfo::default(),
            state: Metadata::default(),
            path: path.clone(),
            next_id: 0,
            storage: HashMap::new(),
            version: 0,
            peers: HashMap::new(),
        };

        entries_id_iter(&path).for_each(|id| {
            let _ = s
                .new_blob_inner(Blob::from_existing(path.join(format!("{}", id))), Some(id))
                .map_err(|e| error!("{:?}", e));
        });

        let config_fut = read_async(path.join(".json")).concat2().and_then(|a| {
            serde_json::from_slice::<Metadata>(a.as_ref()).map_err(|e| e.to_string())
        });

        info_fut.join(config_fut).and_then(|(info, state)| {
            s.info = info;
            s.state = state;
            Ok(s)
        })
    }

    pub fn info(&self) -> SessionInfo {
        self.info.clone()
    }

    pub fn metadata(&self) -> &Metadata {
        &self.state
    }

    pub fn set_metadata(&mut self, val: Metadata) -> impl Future<Item = u64, Error = SessionErr> {
        if self.state.version == val.version {
            self.state = val;
            self.state.version += 1;
        } else {
            return futures::future::Either::B(Err(SessionErr::OverwriteError).into_future());
        }
        self.version += 1;

        let new_state_version = self.state.version;

        futures::future::Either::A(
            write_async(
                stream::once::<_, ()>(Ok(Bytes::from(serde_json::to_vec(&self.state).unwrap()))),
                self.path.join(".json"),
            )
            .map_err(|e| SessionErr::FileError(e))
            .and_then(move |_| Ok(new_state_version)),
        )
    }

    fn new_blob_inner(&mut self, blob: Blob, id: Option<u64>) -> Result<(u64, Blob), SessionErr> {
        let id = match id {
            None => self.next_id,
            Some(v) => v,
        };
        self.next_id = cmp::max(id, self.next_id) + 1;
        self.version += 1;

        match self.storage.insert(id, blob.clone()) {
            Some(_) => Err(SessionErr::OverwriteError),
            None => Ok((id, blob)),
        }
    }

    pub fn new_blob(&mut self) -> Result<(u64, Blob), SessionErr> {
        let blob = Blob::new(self.path.join(format!("{}", self.next_id)))
            .map_err(|e| SessionErr::FileError(e.to_string()))?;
        self.new_blob_inner(blob, None)
    }

    pub fn set_blob(&mut self, id: u64, blob: Blob) -> SessionResult {
        self.version += 1;
        match self.storage.insert(id, blob) {
            Some(_) => Ok(SessionOk::Ok),
            None => Ok(SessionOk::Ok),
        }
    }

    pub fn get_blob(&self, id: u64) -> SessionResult {
        match self.storage.get(&id) {
            Some(b) => Ok(SessionOk::Blob(b.clone())),
            None => Err(SessionErr::BlobNotFoundError),
        }
    }

    pub fn delete_blob(&mut self, id: u64) -> SessionResult {
        self.version += 1;
        match self.storage.remove(&id).map(|b| b.clean_file()) {
            Some(Ok(())) => Ok(SessionOk::Ok),
            Some(Err(e)) => Err(SessionErr::FileError(e.to_string())),
            None => Ok(SessionOk::BlobAlreadyDeleted),
        }
    }

    /*pub fn get_blob_path(&self, id: u64) -> Result<&Path, SessionErr> {
        self.storage
            .get(&id)
            .map(|b| b.path())
            .ok_or(SessionErr::BlobNotFoundError)
    }*/

    pub fn list_blobs(&self) -> Vec<BlobInfo> {
        self.storage
            .keys()
            .map(|e| BlobInfo { id: e.to_string() })
            .collect()
    }

    pub fn list_peers(&self) -> Vec<String> {
        self.peers.keys().map(|e| e.to_string()).collect()
    }

    pub fn add_peers(&mut self, peers: Vec<NodeId>) {
        let new_peers = peers
            .into_iter()
            .filter(|p| !self.peers.get(p).is_none())
            .map(|peer| (peer, PeerState::default()))
            .collect::<Vec<_>>();
        self.peers.extend(new_peers);
    }

    pub fn remove_deployment(&mut self, node_id: NodeId, deployment_id: String) -> bool {
        match self.peers.get_mut(&node_id) {
            None => false,
            Some(peer) => peer.deployments.remove(&deployment_id),
        }
    }

    pub fn add_deployment(&mut self, node_id: NodeId, deployment_id: String) {
        let _ = self
            .peers
            .get_mut(&node_id)
            .and_then(|node_info| Some(node_info.deployments.insert(deployment_id)));
    }

    pub fn create_deployment(
        &mut self,
        node_id: NodeId,
        body: gu_model::envman::CreateSession,
    ) -> impl Future<Item = String, Error = SessionErr> {
        if self.peers.get(&node_id).is_none() {
            return future::Either::A(future::err(SessionErr::NodeNotFound(node_id)));
        }
        future::Either::B(
            peer(node_id)
                .into_endpoint()
                .send(body)
                .map_err(|_| SessionErr::CannotCreatePeerDeployment)
                .and_then(|v| {
                    future::result(v).map_err(|_| SessionErr::CannotCreatePeerDeployment)
                }),
        )
    }

    pub fn delete_deployment(
        &mut self,
        node_id: NodeId,
        deployment_id: String,
    ) -> impl Future<Item = (), Error = SessionErr> {
        if self.peers.get(&node_id).is_none() {
            return future::Either::A(future::err(SessionErr::NodeNotFound(node_id)));
        }
        future::Either::B(
            peer(node_id)
                .into_endpoint()
                .send(gu_model::envman::DestroySession {
                    session_id: deployment_id,
                })
                .map_err(|_| SessionErr::CannotDeletePeerDeployment)
                .and_then(|r| {
                    future::result(r)
                        .map(|_| ())
                        .map_err(|_| SessionErr::CannotDeletePeerDeployment)
                }),
        )
    }

    pub fn update_deployment(
        &mut self,
        node_id: NodeId,
        deployment_id: String,
        commands: Vec<gu_model::envman::Command>,
    ) -> impl Future<Item = (), Error = SessionErr> {
        if self.peers.get(&node_id).is_none() {
            return future::Either::A(future::err(SessionErr::NodeNotFound(node_id)));
        }
        future::Either::B(
            peer(node_id)
                .into_endpoint()
                .send(gu_model::envman::SessionUpdate {
                    session_id: deployment_id,
                    commands: commands,
                })
                .map_err(|_| SessionErr::CannotUpdatePeerDeployment)
                .and_then(|r| {
                    future::result(r)
                        .map(|_| ())
                        .map_err(|_| SessionErr::CannotUpdatePeerDeployment)
                }),
        )
    }

    pub fn clean_directory(&mut self) -> io::Result<()> {
        self.version += 1;
        match (&self.path).exists() {
            true => fs::remove_dir_all(&self.path),
            false => Ok(()),
        }
    }
}
