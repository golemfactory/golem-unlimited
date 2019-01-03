#![warn(dead_code)]

use std::collections::HashSet;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs::DirBuilder;
use std::io;
use std::iter::FromIterator;
use std::fs;

pub trait Volume {
    fn path(&self) -> String;
}

#[derive(Clone)]
pub struct Workspace {
    name: String,
    path: PathBuf,
    metadata: Value,
    tags: HashSet<String>,
    volumes: HashSet<String>,
}

impl Workspace {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            metadata: Value::Null,
            tags: HashSet::new(),
            volumes: HashSet::new(),
        }
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_tags(&self) -> Vec<String> {
        Vec::from_iter(self.tags.clone().into_iter())
    }

    pub fn add_tags<T: IntoIterator<Item=String>>(&mut self, tags: T) {
        for tag in tags {
            self.tags.insert(tag);
        }
    }

    pub fn remove_tags<T: IntoIterator<Item=String>>(&mut self, tags: T) {
        for tag in tags {
            self.tags.remove(&tag);
        }
    }

    pub fn get_metadata(&self) -> &Value {
        &self.metadata
    }

    pub fn set_metadata(&mut self, val: Value) {
        self.metadata = val;
    }

    pub fn add_volume(&mut self, s: String) {
        self.volumes.insert(s);
    }

    pub fn remove_volume(&mut self, s: &String) {
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
            result = builder.create(self.path.join(dir)).and_then(|_| result);
        };

        result
    }

    pub fn clear_dir(&self) -> io::Result<()> {
        debug!("cleaning session dir {:?}", self.path);
        fs::remove_dir_all(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use workspace::Workspace;
    use std::path::Path;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn create_dirs() {
        let path = "/tmp/gu-unlimited/tests";
        let mut work = Workspace::new("work".to_string(),path.into());

        work.add_volume("test1".to_string());
        work.add_volume("test2".to_string());

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
        assert_eq!(work.get_tags(), tags);

        work.remove_tags(["tag1".to_string()].to_vec());
        assert_eq!(work.get_tags(), ["tag2".to_string()].to_vec());
    }
}
