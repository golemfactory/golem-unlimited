use serde::{Deserialize, Serialize};

use super::error::Result;

#[cfg(feature = "clinfo")]
pub use self::clinfo::Error as ClError;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct GpuCount {
    pub amd: u8,
    pub nvidia: u8,
    pub intel: u8,
    pub other: u8,
}

#[cfg(target_os = "linux")]
#[cfg(not(feature = "clinfo"))]
mod linux_pci_scan;

#[cfg(feature = "clinfo")]
mod clinfo;

#[cfg(target_os = "linux")]
#[cfg(not(feature = "clinfo"))]
pub fn gpu_count() -> Result<GpuCount> {
    use self::linux_pci_scan::*;

    Ok(pci_devices()
        .map_err(|e| super::error::Error::Io(e))?
        .filter_map(|device_ref| device_ref.ok())
        .filter(|device| match device.class_code() {
            Ok(code) => code == CL_DEVICE_TYPE_GPU || code == CL_DEVICE_TYPE_ACCELERATOR,
            Err(_) => false,
        })
        .fold(GpuCount::default(), |gpu, device| {
            match device.vendor_code() {
                Ok(VENDOR_CODE_AMD) => GpuCount {
                    amd: gpu.amd + 1,
                    ..gpu
                },
                Ok(VENDOR_CODE_NVIDIA) => GpuCount {
                    nvidia: gpu.nvidia + 1,
                    ..gpu
                },
                Ok(VENDOR_CODE_INTEL) => GpuCount {
                    intel: gpu.intel + 1,
                    ..gpu
                },
                Ok(_) => GpuCount {
                    other: gpu.other + 1,
                    ..gpu
                },
                Err(_) => unimplemented!(),
            }
        }))
}

#[cfg(feature = "clinfo")]
pub fn gpu_count() -> Result<GpuCount> {
    use self::clinfo::*;

    Ok(Platforms::try_new()?
        .filter_map(|platform| platform.devices().ok())
        .flatten()
        .fold(GpuCount::default(), |mut gpu, device| {
            let vendor = device.vendor();
            if vendor.starts_with("Intel") {
                gpu.intel += 1;
            } else if vendor == "AMD" {
                gpu.amd += 1;
            } else if vendor == "NVIDIA Corporation" {
                gpu.nvidia += 1;
            } else {
                eprintln!("vendor={}", vendor);
            }

            gpu
        }))
}

#[cfg(not(target_os = "linux"))]
#[cfg(not(feature = "clinfo"))]
pub fn gpu_count() -> Result<GpuCount> {
    Ok(GpuCount {
        amd: 0,
        nvidia: 0,
        intel: 0,
        other: 0,
    })
    // compile_error!("gpu detection supported only on ubuntu or with clinfo feature")
}

#[cfg(test)]
mod test {
    #[cfg(any(target_os = "linux", feature = "clinfo"))]
    use super::gpu_count;

    #[test]
    #[cfg(any(target_os = "linux", feature = "clinfo"))]
    fn test_gpu_count() {
        eprintln!("gpu={:?}", gpu_count().unwrap());
    }

}
