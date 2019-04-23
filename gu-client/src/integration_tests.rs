use actix::prelude::*;
use futures::future;
use futures::prelude::*;

use crate::r#async::*;
use gu_model::session::HubSessionSpec;
use gu_net::rpc::ws::start_connection;

#[test]
fn test_list_peers() {
    let mut sys = System::new("test");

    let connection = HubConnection::default();

    let alloc_peers = connection
        .list_peers()
        .and_then(move |peers| {
            connection
                .new_session(HubSessionSpec::default())
                .and_then(move |session| Ok((connection, session, peers)))
        })
        .and_then(|(connection, session, peers)| {
            let peers: Vec<_> = peers.collect();
            session
                .add_peers(peers.clone().into_iter().map(|p| p.node_id))
                .and_then(|_| {
                    session.list_peers().and_then(|session_peers| {
                        Ok((peers, session_peers.collect::<Vec<_>>(), session))
                    })
                })
        })
        .and_then(|(peers, session_peers, session)| {
            eprintln!("checking session peers");
            let session_it = session.clone();

            futures::future::join_all(
                session_peers
                    .into_iter()
                    .map(move |peer| session_it.peer(peer.node_id).info()),
            )
            .and_then(|peers_details| Ok((peers_details, session)))
        });

    let (peers, session) = sys.block_on(alloc_peers).unwrap();

    eprintln!("peers={:?}", peers);
    //    assert_eq!(peers.len(), session_peers.len());
}

/// Test if two ways of getting PeerInfo are consistent
#[test]
fn test_peer_info() {
    let mut sys = System::new("test");
    let connection = HubConnection::default();
    let fut = connection.list_peers().and_then(move |mut peers| {
        let peerinfo = peers.next().unwrap();
        let node_id = peerinfo.node_id;
        let peerinfo2 = connection.peer(node_id).info();
        future::ok(peerinfo).join(peerinfo2)
    });

    let (pi, pi2) = sys.block_on(fut).unwrap();
    assert_eq!(pi.node_id, pi2.node_id);
}
