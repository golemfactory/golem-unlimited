use std::{
    fs::{self, ReadDir},
    io::{self, Read},
    path, str,
};

pub type VendorCode = u16;

pub const VENDOR_CODE_AMD: VendorCode = 0x1002;
pub const VENDOR_CODE_NVIDIA: VendorCode = 0x10de;
pub const VENDOR_CODE_INTEL: VendorCode = 0x8086;
pub const CL_DEVICE_TYPE_GPU: u32 = 0x030000;
pub const CL_DEVICE_TYPE_ACCELERATOR: u32 = 0x030200;

pub struct PciDevices {
    inner: ReadDir,
}

pub fn pci_devices() -> io::Result<PciDevices> {
    let inner = fs::read_dir("/sys/bus/pci/devices")?;

    Ok(PciDevices { inner })
}

impl Iterator for PciDevices {
    type Item = Result<PciDevice, io::Error>;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        loop {
            let dir_entry = match self.inner.next() {
                None => return None,
                Some(Err(e)) => return Some(Err(e)),
                Some(Ok(path)) => path,
            };

            let file_type = match dir_entry.file_type() {
                Ok(file_type) => file_type,
                Err(e) => return Some(Err(e)),
            };

            if file_type.is_symlink() || file_type.is_dir() {
                return Some(Ok(PciDevice {
                    inner: dir_entry.path(),
                }));
            }
        }
    }
}

pub struct PciDevice {
    inner: path::PathBuf,
}

impl PciDevice {
    fn decode_hex(&self, attr: &str) -> io::Result<i64> {
        let mut buf = [0u8; 20];
        let mut f = fs::OpenOptions::new()
            .read(true)
            .open(self.inner.join(attr))?;
        let n = f.read(&mut buf)?;
        if n >= buf.len() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        match str::from_utf8(&buf[0..n - 1]) {
            Ok(s) => {
                let s = s.trim_end();
                if &s[0..2] != "0x" {
                    return Err(io::ErrorKind::InvalidInput.into());
                }

                i64::from_str_radix(&s[2..], 16).map_err(|_e| io::ErrorKind::InvalidInput.into())
            }
            Err(_e) => Err(io::ErrorKind::InvalidInput.into()),
        }
    }

    pub fn class_code(&self) -> io::Result<u32> {
        self.decode_hex("class").and_then(|code| Ok(code as u32))
    }

    pub fn vendor_code(&self) -> io::Result<u16> {
        self.decode_hex("vendor").and_then(|code| Ok(code as u16))
    }
}
