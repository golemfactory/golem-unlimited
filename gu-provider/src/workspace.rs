#![allow(dead_code)]

use gu_model::dockerman::VolumeDef;
use gu_persist::config::ConfigModule;
use provision::untgz;
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::fs::DirBuilder;
use std::io;
use std::iter::FromIterator;
use std::path::PathBuf;
use uuid::Uuid;

pub struct WorkspacesManager {
    namespace: &'static str,
    path: PathBuf,
}

impl WorkspacesManager {
    pub fn new(config: &ConfigModule, name: &'static str) -> Option<WorkspacesManager> {
        let sessions_dir = config.work_dir().to_path_buf().join("sessions");

        fs::create_dir_all(&sessions_dir)
            .map_err(|e| error!("Cannot create HdMan dir: {:?}", e))
            .map(|_| WorkspacesManager {
                namespace: name,
                path: sessions_dir,
            })
            .ok()
    }

    pub fn workspace(&self, name: String) -> Workspace {
        let dir_name = format!("{}::{}", self.namespace, Uuid::new_v4());
        Workspace::new(name, self.path.join(dir_name))
    }
}

type Set<K> = BTreeSet<K>;

#[derive(Clone)]
pub struct Workspace {
    name: String,
    path: PathBuf,
    metadata: Value,
    tags: Set<String>,
    volumes: HashSet<VolumeDef>,
}

impl Workspace {
    pub(self) fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            metadata: Value::Null,
            tags: Set::new(),
            volumes: HashSet::new(),
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn tags(&self) -> Vec<String> {
        Vec::from_iter(self.tags.iter().cloned())
    }

    pub fn add_tags<T: IntoIterator<Item = String>>(&mut self, tags: T) {
        for tag in tags {
            self.tags.insert(tag);
        }
    }

    pub fn remove_tags<T: IntoIterator<Item = String>>(&mut self, tags: T) {
        for tag in tags {
            self.tags.remove(&tag);
        }
    }

    pub fn metadata(&self) -> &Value {
        &self.metadata
    }

    pub fn set_metadata(&mut self, val: Value) {
        self.metadata = val;
    }

    pub fn add_volume(&mut self, s: VolumeDef) {
        self.volumes.insert(s);
    }

    pub fn remove_volume(&mut self, s: &VolumeDef) {
        self.volumes.remove(s);
    }

    /// Creates dirs that are included in inner volumes list
    /// They are created as children of a directory provided on Workspace creation
    pub fn create_dirs(&self) -> io::Result<()> {
        let mut builder = DirBuilder::new();
        builder.recursive(true);

        debug!("creating work dir {:?}", self.path);
        let mut result = builder.create(self.path.to_path_buf());
        for dir in self.volumes.iter() {
            match dir.source_dir() {
                Some(dir) => {
                    result = builder.create(self.path.join(dir)).and_then(|_| result);
                }
                _ => (),
            }
        }

        result
    }

    pub fn clear_dir(&self) -> io::Result<()> {
        debug!("cleaning session dir {:?}", self.path);
        fs::remove_dir_all(&self.path)
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use gu_model::dockerman::VolumeDef;
    use std::path::PathBuf;
    use workspace::Workspace;

    #[test]
    fn create_dirs() {
        let path = "/tmp/gu-unlimited/tests";
        let mut work = Workspace::new("work".to_string(), path.into());

        work.add_volume(VolumeDef::BindRw {
            src: "test1".to_string(),
            target: "".to_string(),
        });
        work.add_volume(VolumeDef::BindRw {
            src: "test2".to_string(),
            target: "".to_string(),
        });

        work.create_dirs().unwrap();

        assert!(&PathBuf::from(path).join("test1").exists());
        assert!(&PathBuf::from(path).join("test2").exists());
    }

    #[test]
    fn tags() {
        let path = "/tmp/gu-unlimited/tests";
        let mut work = Workspace::new("work".to_string(), path.into());
        let tags = ["tag1".to_string(), "tag2".to_string()].to_vec();

        work.add_tags(tags.clone());
        work.add_tags(["tag1".to_string()].to_vec());
        assert_eq!(work.tags(), tags);

        work.remove_tags(["tag1".to_string()].to_vec());
        assert_eq!(work.tags(), ["tag2".to_string()].to_vec());
    }
}
