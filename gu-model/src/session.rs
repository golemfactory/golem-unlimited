use super::Map;
use super::Tags;
use chrono::prelude::*;
use chrono::DateTime;
use serde_derive::*;
use serde_json::Value as JsonValue;

#[derive(Serialize, Deserialize)]
pub struct HubSessionUpdate {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<DateTime<Utc>>,
    #[serde(flatten)]
    pub command: Command,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "commandType")]
pub enum Command {
    #[serde(rename = "HubSessionTouchCommand")]
    #[serde(rename_all = "camelCase")]
    Touch { keep_until: Option<DateTime<Utc>> },
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct HubSessionSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<DateTime<Utc>>,
    #[serde(default)]
    pub allocation: AllocationMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub tags: Tags,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HubExistingSession {
    pub id: u64,
    pub created: DateTime<Utc>,
    #[serde(flatten)]
    pub spec: HubSessionSpec,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum AllocationMode {
    #[serde(rename = "auto")]
    AUTO,
    #[serde(rename = "manual")]
    MANUAL,
}

impl Default for AllocationMode {
    fn default() -> Self {
        AllocationMode::MANUAL
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(default)]
    pub version: u64,
    #[serde(flatten)]
    pub entry: Map<String, JsonValue>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetails {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<DateTime<Utc>>,
    #[serde(default)]
    pub allocation: AllocationMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub tags: Tags,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BlobInfo {
    pub id: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_metadata() {
        let m0 = r#"{"version": 0, "ala": 10, "kot": {"a": 89}}"#;

        let j0: Metadata = serde_json::from_str(m0).unwrap();

        assert_eq!(j0.version, 0);
        assert_eq!(j0.entry.get("ala").unwrap().as_i64().unwrap(), 10);
        assert_eq!(
            j0.entry
                .get("kot")
                .unwrap()
                .get("a")
                .unwrap()
                .as_i64()
                .unwrap(),
            89
        );

        let m1 = Metadata {
            version: 1,
            entry: vec![
                ("ala".to_owned(), json!(17)),
                ("kot".to_owned(), json!({"a": 7})),
            ]
            .into_iter()
            .collect(),
        };

        let j1 = serde_json::to_string_pretty(&m1).unwrap();

        eprintln!("{}", j1);
    }

    #[test]
    fn test_hub_command() {
        let command = HubSessionUpdate {
            ts: None,
            command: Command::Touch { keep_until: None },
        };
        eprintln!("{}", serde_json::to_string(&command).unwrap());
    }

}
