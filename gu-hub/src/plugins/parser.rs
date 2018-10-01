use bytes::Bytes;
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
use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::path::Path;
use std::path::PathBuf;
use zip::ZipArchive;

pub trait PluginParser: Debug {
    /// Performs all checks on plugin resource and returns its metadata on success
    fn validate_and_load_metadata(
        &mut self,
        gu_version: Version,
    ) -> Result<PluginMetadata, String> {
        let metadata = self.load_metadata()?;

        validate_gu_version(&metadata.gu_version_req(), &gu_version)?;
        self.contains_files(metadata.load())?;

        Ok(metadata)
    }

    /// Loads to memory all files from app_name sub-resource
    fn load_files(&mut self, app_name: &str) -> Result<HashMap<PathBuf, Vec<u8>>, String>;

    /// Parses gu-plugin.json file
    fn load_metadata(&mut self) -> Result<PluginMetadata, String>;

    /// Checks if plugin source contains files from metadata load field
    fn contains_files(&mut self, files: &Vec<String>) -> Result<(), String> {
        let meta = self.load_metadata()?;
        let mut app_name = meta.name();

        files.into_iter().fold(Ok(()), |init, name| {
            init.and_then(|()| {
                let mut path = PathBuf::from(app_name.clone());
                path.push(name);
                self.contains_file(path)
            }).and_then(|_| Ok(()))
        })
    }

    /// Checks if plugin source contains file on relative `path`
    fn contains_file(&mut self, path: PathBuf) -> Result<(), String>;
}

pub fn parse_metadata(metadata_file: impl Read) -> Result<PluginMetadata, String> {
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
    file.read_to_end(&mut buf)
        .map_err(|e| format!("Error during file read: {:?}", e))?;
    Ok(buf)
}

pub trait PathPluginParser: PluginParser + Sized {
    fn from_path(path: &Path) -> Result<Self, String>;
}

pub trait BytesPluginParser: PluginParser + Sized {
    fn from_bytes(bytes: Cursor<Bytes>) -> Result<Self, String>;
}

#[derive(Debug)]
pub struct ZipParser<T: Debug + Read + Seek> {
    archive: ZipArchive<T>,
}

impl BytesPluginParser for ZipParser<BufReader<Cursor<Bytes>>> {
    fn from_bytes(bytes: Cursor<Bytes>) -> Result<Self, String> {
        let buf_reader = BufReader::new(bytes);
        let archive = ZipArchive::new(buf_reader)
            .map_err(|e| format!("Cannot parse string as zip: {:?}", e))?;

        Ok(ZipParser::inner_new(archive))
    }
}

impl PathPluginParser for ZipParser<File> {
    fn from_path(path: &Path) -> Result<Self, String> {
        File::open(path)
            .map_err(|e| format!("Cannot open file: {:?}", e))
            .and_then(|f| {
                ZipArchive::new(f).map_err(|e| format!("Cannot open file as zip: {:?}", e))
            }).map(|zip| Self::inner_new(zip))
    }
}

impl<T: Debug + Read + Seek> ZipParser<T> {
    fn inner_new(zip: ZipArchive<T>) -> Self {
        Self { archive: zip }
    }
}

impl<T: Read + Debug + Seek> PluginParser for ZipParser<T> {
    fn load_files(&mut self, app_name: &str) -> Result<HashMap<PathBuf, Vec<u8>>, String> {
        let mut map = HashMap::new();

        for i in 0..self.archive.len() {
            let file = self.archive.by_index(i);

            if file.is_err() {
                warn!("Error during reading zip: {:?}", file.err().unwrap());
                continue;
            }
            let mut file = file.unwrap();

            match file.sanitized_name().strip_prefix(app_name) {
                Err(e) => (),
                Ok(path) => {
                    let _ = read_file(&mut file)
                        .and_then(|x| {
                            map.insert(path.to_path_buf(), x)
                                .map(|_| warn!("File overwritten: {:?}", path));
                            Ok(())
                        }).map_err(|e| warn!("{:?}", e));
                }
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

    fn contains_file(&mut self, path: PathBuf) -> Result<(), String> {
        self.archive
            .by_name(
                path.to_str()
                    .ok_or("Cannot cast PathBuf to str".to_string())?,
            ).map_err(|_| "".to_string())
            .and_then(|_| Ok(()))
    }
}

/*
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
            let _ = el
                .and_then(|a| a.file_type().map(|f_type| (f_type, a)))
                .map_err(|e| warn!("Couldn't read filesystem element: {:?}", e))
                .and_then(&mut handle_entry);
        })
    }
}

impl PluginParser for DirectoryParser {
    fn load_files(&mut self, app_name: &str) -> Result<HashMap<PathBuf, Vec<u8>>, String> {
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

    fn contains_file(&mut self, path: PathBuf) -> Result<(), String> {
        File::open(self.path.to_path_buf().join(path).join("app.js"))
            .map_err(|_| "Cannot read {} file".to_string())
            .and_then(|_| Ok(()))
    }
}
*/
