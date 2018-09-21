use std::path::Path;
use zip::ZipArchive;
use std::fs::File;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::io::Read;
use semver::Version;
use serde_json;
use plugin::plugin::PluginMetadata;

pub fn validate_and_load_metadata(path: &Path, gu_version: Version) -> Result<(String, PluginMetadata), String> {
    // Validate zip filename
    let zip_name = path
        .file_name()
        .ok_or_else(|| format!("Cannot get zip archive name"))?
        .to_str()
        .ok_or_else(|| format!("Invalid unicode in zip archive name"))?;

    let metadata = extract_metadata(path)?;
    if metadata.proper_version(&gu_version) {
        return Err(format!(
            "Too low gu-app version ({}). Required {}",
            gu_version, metadata.version()
        ));
    }
    contains_app_js(&path, &metadata.name())?;

    Ok((zip_name.to_string(), metadata))
}

fn open_archive(path: &Path) -> Result<ZipArchive<File>, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open archive: {:?}", e))?;
    ZipArchive::new(file).map_err(|e| format!("Cannot unzip file: {:?}", e))
}

fn extract_metadata(path: &Path) -> Result<PluginMetadata, String> {
    let mut archive = open_archive(path)?;

    let metadata_file = archive
        .by_name("gu-plugin.json")
        .map_err(|e| format!("Cannot read gu-plugin.json file: {:?}", e))?;

    serde_json::from_reader(metadata_file)
        .map_err(|e| format!("Cannot parse gu-plugin.json file: {:?}", e))
}

fn contains_app_js(path: &Path, name: &String) -> Result<(), String> {
    let mut archive = open_archive(path)?;
    let mut app_name = name.clone();
    app_name.push_str("/app.js");

    archive
        .by_name(app_name.as_ref())
        .map_err(|e| format!("Cannot read {} file: {:?}", app_name, e))?;

    Ok(())
}

pub fn load_archive(
    zip_path: &Path,
    app_name: &String,
) -> Result<HashMap<PathBuf, Arc<Vec<u8>>>, String> {
    let mut archive = open_archive(zip_path)?;
    let mut map = HashMap::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| warn!("Error during unzip: {:?}", e));

        if file.is_err() {
            continue;
        }

        let mut file = file.unwrap();
        let out_path = file.sanitized_name();

        if out_path.starts_with(app_name) {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf);
            map.insert(out_path, Arc::new(buf));
        }
    }

    Ok(map)
}
