use plugins::plugin::PluginMetadata;
use semver::Version;
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

pub trait PluginParser: Debug + PluginParserInner {
    fn validate_and_load_metadata(
        path: &Path,
        gu_version: Version,
    ) -> Result<(String, PluginMetadata), String> {
        let name = Self::validate_package_name(path)?;
        let metadata = Self::load_metadata(path)?;
        Self::validate_gu_version(&metadata, &gu_version)?;
        Self::contains_app_js(&path, &metadata.name())?;

        Ok((name, metadata))
    }

    fn load_files(zip_path: &Path, app_name: &String) -> Result<HashMap<PathBuf, Vec<u8>>, String>;
}

pub trait PluginParserInner: Debug {
    fn validate_package_name(path: &Path) -> Result<String, String> {
        path.file_name()
            .ok_or_else(|| format!("Cannot get package name"))?
            .to_str()
            .ok_or_else(|| format!("Invalid unicode in package name"))
            .map(|x| x.to_string())
    }

    fn load_metadata(path: &Path) -> Result<PluginMetadata, String>;

    fn contains_app_js(path: &Path, name: &String) -> Result<(), String>;

    fn parse_metadata(metadata_file: impl Read) -> Result<PluginMetadata, String> {
        serde_json::from_reader(metadata_file)
            .map_err(|e| format!("Cannot parse gu-plugin.json file: {:?}", e))
    }

    fn validate_gu_version(metadata: &PluginMetadata, gu_version: &Version) -> Result<(), String> {
        if !metadata.proper_version(gu_version) {
            Err(format!("Too low gu-app version ({})", gu_version))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Default)]
pub struct ZipParser;

impl ZipParser {
    fn open_archive(path: &Path) -> Result<ZipArchive<File>, String> {
        let file =
            File::open(path).map_err(|e| format!("Cannot open {:?} archive: {:?}", path, e))?;
        ZipArchive::new(file).map_err(|e| format!("Cannot unzip file: {:?}", e))
    }
}

impl PluginParserInner for ZipParser {
    fn load_metadata(path: &Path) -> Result<PluginMetadata, String> {
        let mut archive = Self::open_archive(path)?;

        let metadata_file = archive
            .by_name("gu-plugin.json")
            .map_err(|e| format!("Cannot read gu-plugin.json file: {:?}", e))?;

        Self::parse_metadata(metadata_file)
    }

    fn contains_app_js(path: &Path, name: &String) -> Result<(), String> {
        let mut archive = Self::open_archive(path)?;
        let mut app_name = name.clone();
        app_name.push_str("/app.js");

        archive
            .by_name(app_name.as_ref())
            .map_err(|e| format!("Cannot read {} file: {:?}", app_name, e))?;

        Ok(())
    }
}

impl PluginParser for ZipParser {
    fn load_files(zip_path: &Path, app_name: &String) -> Result<HashMap<PathBuf, Vec<u8>>, String> {
        let mut archive = Self::open_archive(zip_path)?;
        let mut map = HashMap::new();

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| warn!("Error during unzip: {:?}", e));

            if file.is_err() {
                continue;
            }

            let mut file = file.map_err(|e| format!("Cannot read file: {:?}", e))?;
            let out_path = file.sanitized_name();

            match out_path.strip_prefix(app_name) {
                Ok(path) => {
                    map.insert(path.to_path_buf(), read_file(&mut file));
                }
                _ => (),
            }
        }

        Ok(map)
    }
}

#[derive(Debug, Default)]
pub struct DirectoryParser;

impl DirectoryParser {
    fn scan_dir(entry: &mut ReadDir, path: &mut PathBuf, map: &mut HashMap<PathBuf, Vec<u8>>) {
        let mut handle_entry = |(f_type, entry): (FileType, DirEntry)| {
            path.push(entry.file_name());
            let _ = if f_type.is_file() {
                File::open(entry.path())
                    .as_mut()
                    .map(|read| map.insert(path.clone(), read_file(read)))
                    .map_err(|e| warn!("Couldn't read file: {:?}", e));
            } else if f_type.is_dir() {
                read_dir(entry.path())
                    .as_mut()
                    .map(|read| Self::scan_dir(read, path, map))
                    .map_err(|e| warn!("Couldn't read directory: {:?}", e));
            };
            path.pop();

            Ok(())
        };

        entry.for_each(|el| {
            el.and_then(|a| a.file_type().map(|f_type| (f_type, a)))
                .map_err(|e| format!("Couldn't read filesystem element: {:?}", e))
                .and_then(&mut handle_entry);
        })
    }
}

impl PluginParserInner for DirectoryParser {
    fn load_metadata(path: &Path) -> Result<PluginMetadata, String> {
        let metadata_file = File::open(path.to_path_buf().join("gu-plugin.json"))
            .map_err(|_| "Couldn't read metadata file".to_string())?;

        Self::parse_metadata(metadata_file)
    }

    fn contains_app_js(path: &Path, name: &String) -> Result<(), String> {
        File::open(path.to_path_buf().join(name).join("app.js"))
            .map_err(|_| "Couldn't read app.js file".to_string())?;

        Ok(())
    }
}

fn read_file(file: &mut impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    file.read_to_end(&mut buf);
    buf
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