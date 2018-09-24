use plugins::plugin::PluginMetadata;
use semver::Version;
use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use zip::ZipArchive;
use std::fmt::Debug;

pub trait PluginParser: Debug {
    fn validate_and_load_metadata(path: &Path, gu_version: Version)
        -> Result<(String, PluginMetadata), String>;

    fn load_files(zip_path: &Path, app_name: &String) -> Result<HashMap<PathBuf, Vec<u8>>, String>;
}

impl<T> PluginParser for T
where T: PluginParserInner {
    fn validate_and_load_metadata(path: &Path, gu_version: Version)
                                  -> Result<(String, PluginMetadata), String> {
        let name = T::validate_package_name(path)?;
        let metadata = T::load_metadata(path)?;
        T::validate_gu_version(&metadata, &gu_version)?;
        T::contains_app_js(&path, &metadata.name())?;

        Ok((name, metadata))
    }

    fn load_files(zip_path: &Path, app_name: &String) -> Result<HashMap<PathBuf, Vec<u8>>, String> {
        T::load_files(zip_path, app_name)
    }
}

pub trait PluginParserInner: Debug {
    fn validate_package_name(path: &Path) -> Result<String, String> {
        path
            .file_name()
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

    fn load_files(zip_path: &Path, app_name: &String) -> Result<HashMap<PathBuf, Vec<u8>>, String>;
}

#[derive(Debug, Default)]
pub struct ZipParser;

impl ZipParser {
    fn open_archive(path: &Path) -> Result<ZipArchive<File>, String> {
        let file = File::open(path).map_err(|e| format!("Cannot open {:?} archive: {:?}", path, e))?;
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

fn read_file(file: &mut impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    file.read_to_end(&mut buf);
    buf
}