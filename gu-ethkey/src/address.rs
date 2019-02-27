use std::fmt;
use rustc_hex::ToHex;

/// Ethereum address
pub struct Address([u8; 20]);

impl Address {
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl From<&[u8]> for Address {
    fn from(val: &[u8]) -> Self {
        let mut address = [0u8; 20];
        address.copy_from_slice(val);
        Address(address)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(fmt, "0x{}", self.0.to_hex::<String>())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(fmt, "Address({})", self)
    }
}


#[cfg(test)]
mod tests {
    use rustc_hex::FromHex;
    use crate::Address;

    #[test]
    fn address_should_have_debug_impl() {
        let raw: Vec<u8> = "60f0dc62f0fac30a5beee9ac998590026923aa79".from_hex().unwrap();
        let addr = Address::from(raw.as_ref());

        assert_eq!(format!("{:?}", addr), "Address(0x60f0dc62f0fac30a5beee9ac998590026923aa79)");
    }

    #[test]
    fn address_should_have_display_impl() {
        let raw: Vec<u8> = "60f0dc62f0fac30a5beee9ac998590026923aa79".from_hex().unwrap();
        let addr = Address::from(raw.as_ref());

        assert_eq!(format!("{}", addr), "0x60f0dc62f0fac30a5beee9ac998590026923aa79");
    }
}