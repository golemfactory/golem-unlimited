use plugins::plugin::PluginMetadata;
use semver::Version;
use semver::VersionReq;
use serde_json;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::read_dir;
use std::fs::DirEntry;
use std::fs::File;
use std::fs::FileType;
use std::fs::ReadDir;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use zip::ZipArchive;
use std::io::Seek;
use bytes::Bytes;

pub trait PluginParser: Debug {
    /// Performs all checks on plugin resource and returns its metadata on success
    fn validate_and_load_metadata(&mut self, gu_version: Version) -> Result<PluginMetadata, String> {
        let metadata = self.load_metadata()?;

        validate_gu_version(&metadata.gu_version_req(), &gu_version)?;
        self.contains_app_js(&metadata.name())?;

        Ok(metadata)
    }

    /// Loads to memory all files from app_name sub-resource
    fn load_files(&mut self, app_name: &String) -> Result<HashMap<PathBuf, Vec<u8>>, String>;

    /// Parses gu-plugin.json file
    fn load_metadata(&mut self) -> Result<PluginMetadata, String>;

    /// Checks if plugin source contains app.js file
    fn contains_app_js(&mut self, app_name: &String) -> Result<(), String>;
}

fn parse_metadata(metadata_file: impl Read) -> Result<PluginMetadata, String> {
    serde_json::from_reader(metadata_file)
        .map_err(|e| format!("Cannot parse gu-plugin.json file: {:?}", e))
}

fn validate_gu_version(metadata: &VersionReq, gu_version: &Version) -> Result<(), String> {
    if metadata.matches(gu_version) {
        Ok(())
    } else {
        Err(format!("Too low gu-app version ({})", gu_version))
    }
}

fn read_file(file: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|e| format!("Error during file read: {:?}", e))?;
    Ok(buf)
}

#[derive(Debug)]
pub struct ZipParser<T: Debug + Read + Seek> {
    archive: ZipArchive<T>,
}


pub trait PathPluginParser: PluginParser + Sized {
    fn new(path: &Path) -> Result<Self, String>;
}

impl PathPluginParser for ZipParser<File> {
    fn new(path: &Path) -> Result<Self, String> {
        File::open(path)
            .map_err(|e| format!("Cannot open file: {:?}", e))
            .and_then(|f| ZipArchive::new(f)
                .map_err(|e| format!("Cannot open file as zip: {:?}", e)))
            .map(|zip| Self::inner_new(zip))
    }
}

impl ZipParser<File> {
    fn inner_new(zip: ZipArchive<File>) -> Self {
        Self {
            archive: zip,
        }
    }
}

impl<T: Read + Debug + Seek> PluginParser for ZipParser<T> {
    fn load_files(&mut self, app_name: &String) -> Result<HashMap<PathBuf, Vec<u8>>, String> {
        let mut map = HashMap::new();

        for i in 0..self.archive.len() {
            let mut file = self
                .archive
                .by_index(i)
                .map_err(|e| warn!("Error during unzip: {:?}", e));

            if file.is_err() {
                continue;
            }

            let mut file = file.map_err(|e| format!("Cannot read file: {:?}", e))?;
            let out_path = file.sanitized_name();

            match out_path.strip_prefix(app_name) {
                Ok(path) => {
                    match read_file(&mut file) {
                        Ok(x) => {
                            map.insert(path.to_path_buf(), x);
                        },
                        Err(e) => warn!("{}", e),
                    }
                }
                _ => (),
            }
        }

        Ok(map)
    }

    fn load_metadata(&mut self) -> Result<PluginMetadata, String> {
        let metadata_file = self
            .archive
            .by_name("gu-plugin.json")
            .map_err(|e| format!("Cannot read gu-plugin.json file: {:?}", e))?;

        parse_metadata(metadata_file)
    }

    fn contains_app_js(&mut self, app_name: &String) -> Result<(), String> {
        let mut app_name = app_name.clone();
        app_name.push_str("/app.js");

        self.archive
            .by_name(app_name.as_ref())
            .map_err(|e| format!("Cannot read {} file: {:?}", app_name, e))?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct DirectoryParser {
    path: Path,
}

impl DirectoryParser {
    fn scan_dir(entry: &mut ReadDir, path: &mut PathBuf, map: &mut HashMap<PathBuf, Vec<u8>>) {
        let mut handle_entry = |(f_type, entry): (FileType, DirEntry)| {
            path.push(entry.file_name());
            if f_type.is_file() {
                let _ = File::open(entry.path())
                    .map_err(|e| format!("Couldn't open file: {:?}", e))
                    .as_mut()
                    .map(|read| read_file(read))
                    .map_err(|e| format!("Couldn't read file: {:?}", e))
                    .and_then(|a| a)
                    .map(|vec| map.insert(path.clone(), vec))
                    .map_err(|e| warn!("{}", e));
            } else if f_type.is_dir() {
                let _ = read_dir(entry.path())
                    .as_mut()
                    .map(|read| Self::scan_dir(read, path, map))
                    .map_err(|e| warn!("Couldn't read directory: {:?}", e));
            };
            path.pop();

            Ok(())
        };

        entry.for_each(|el| {
            let _ = el.and_then(|a| a.file_type().map(|f_type| (f_type, a)))
                .map_err(|e| warn!("Couldn't read filesystem element: {:?}", e))
                .and_then(&mut handle_entry);
        })
    }
}

impl PluginParser for DirectoryParser {
    fn load_files(&mut self, app_name: &String) -> Result<HashMap<PathBuf, Vec<u8>>, String> {
        let mut dir = read_dir(&self.path.join(app_name))
            .map_err(|e| format!("Cannot read directory: {:?}", e))?;
        let mut path = PathBuf::from("");
        let mut map = HashMap::new();

        Self::scan_dir(&mut dir, &mut path, &mut map);

        Ok(map)
    }

    fn load_metadata(&mut self) -> Result<PluginMetadata, String> {
        let metadata_file = File::open(self.path.to_path_buf().join("gu-plugin.json"))
            .map_err(|_| "Couldn't read metadata file".to_string())?;

        parse_metadata(metadata_file)
    }

    fn contains_app_js(&mut self, name: &String) -> Result<(), String> {
        File::open(self.path.to_path_buf().join(name).join("app.js"))
            .map_err(|_| "Couldn't read app.js file".to_string())?;

        Ok(())
    }
}

/*
#[cfg(test)]
mod tests {
    use plugins::zip::DirectoryParser;
    use std::fs::read_dir;
    use std::collections::HashMap;
    use std::path::PathBuf;


    #[test]
    fn it_works() {
        let mut dir = read_dir("/home/hubert/test").unwrap();
        let mut path = PathBuf::from("");
        let mut map = HashMap::new();
        DirectoryParser::scan_dir(&mut dir, &mut path, &mut map);

        println!("{:?}", map)
    }
}
*/
