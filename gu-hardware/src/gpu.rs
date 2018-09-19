#![cfg(target_os = "linux")]

use actix::Message;
use error::Result;
use std::fs::{read_dir, File, ReadDir};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::str::from_utf8;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct GpuCount {
    amd: u8,
    nvidia: u8,
    #[cfg(feature = "intel")]
    intel: u8,
    other: u8,
}

impl GpuCount {
    fn add_amd(&mut self) {
        self.amd += 1;
    }

    fn add_nvidia(&mut self) {
        self.nvidia += 1;
    }

    #[cfg(feature = "intel")]
    fn add_intel(&mut self) {
        self.intel += 1;
    }

    fn add_other(&mut self) {
        self.other += 1;
    }

    pub fn amd(&self) -> u8 {
        self.amd
    }

    pub fn nvidia(&self) -> u8 {
        self.nvidia
    }

    #[cfg(feature = "intel")]
    pub fn intel(&self) -> u8 {
        self.intel
    }

    pub fn other(&mut self) -> u8 {
        self.other
    }
}

// encoded gpu information
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
struct RawGpuInfo {
    vendor: [u8; 4],
}

impl RawGpuInfo {
    fn to_str(&self) -> &str {
        from_utf8(&self.vendor).unwrap_or("<unknown>")
    }
}

fn get_code(path: &PathBuf) -> Result<[u8; 4]> {
    let mut code = [0; 4];
    File::open(path).and_then(|mut f| {
        f.seek(SeekFrom::Start(2))?;
        f.read_exact(&mut code)?;
        Ok(())
    })?;

    Ok(code)
}

fn is_vga_device(path: &mut PathBuf) -> Result<bool> {
    path.push("class");
    let code = get_code(path);
    path.pop();

    Ok(code? == "0300".as_bytes())
}

fn vendor_code(path: &mut PathBuf) -> Result<[u8; 4]> {
    path.push("vendor");
    let code = get_code(path);
    path.pop();

    code
}

fn vga_device(device_dir: &mut PathBuf) -> Result<Option<RawGpuInfo>> {
    Ok(if is_vga_device(device_dir)? {
        Some(RawGpuInfo {
            vendor: vendor_code(device_dir)?,
        })
    } else {
        None
    })
}

fn raw_gpu_list(dir: ReadDir) -> Result<Vec<RawGpuInfo>> {
    let mut gpus = Vec::new();
    for entry in dir {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() || file_type.is_dir() {
            gpus.push(vga_device(&mut entry.path()));
        }
    }

    Ok(gpus
        .into_iter()
        .filter_map(|a| {
            a.unwrap_or_else(|e| {
                warn!("Error during PCI device check: {:?}", e);
                None
            })
        })
        .collect())
}

fn decode_gpu_list(raw_gpus: &Vec<RawGpuInfo>) -> GpuCount {
    let mut counts = GpuCount::default();
    for gpu in raw_gpus {
        match gpu.to_str() {
            "1002" => counts.add_amd(),
            "10de" => counts.add_nvidia(),
            #[cfg(feature = "intel")]
            "8086" => counts.add_intel(),
            _ => counts.add_other(),
        }
    }
    counts
}

pub fn discover_gpu_vendors() -> Result<GpuCount> {
    let list = raw_gpu_list(read_dir("/sys/bus/pci/devices")?)?;
    debug!("List of encoded GPUs: {:?}", list);
    Ok(decode_gpu_list(&list))
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GpuQuery;

impl Message for GpuQuery {
    type Result = Result<GpuCount>;
}
