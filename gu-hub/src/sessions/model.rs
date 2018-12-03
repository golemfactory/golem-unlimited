//! JSON Model for HUB Session API
use chrono::{DateTime, Local};
use gu_net::types::NodeId;
use std::collections::BTreeSet;

type Tags = BTreeSet<String>;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeerInfo {
    node_name: Option<String>,
    peer_addr: String,
    node_id: NodeId,
    #[serde(default)]
    tags: Tags,
}

#[derive(Serialize, Deserialize)]
struct HubSession {
    id: String,
    created: DateTime<Local>,
}

#[cfg(test)]
mod test {

    use super::*;
    use serde_json;

    #[test]
    fn test_serialize_hs() {
        use chrono::prelude::*;
        let dt = Utc.ymd(2014, 7, 8).and_hms(9, 10, 11);

        let v = serde_json::to_value(HubSession {
            id: "id-ud-id".into(),
            created: dt.with_timezone(&Local),
        }).unwrap();

        eprintln!("v= {}", v);
    }

}
