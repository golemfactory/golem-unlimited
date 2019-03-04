use rustc_hex::ToHex;
use std::fmt;

/// Ethereum address
pub struct Address([u8; 20]);

impl Address {
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl From<[u8; 20]> for Address {
    fn from(array: [u8; 20]) -> Self {
        Address(array)
    }
}

impl From<&[u8]> for Address {
    fn from(slice: &[u8]) -> Self {
        let mut address = [0u8; 20];
        address.copy_from_slice(slice);
        Address(address)
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
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
    use crate::Address;
    use rustc_hex::FromHex;

    #[test]
    fn should_convert_to_vec() {
        let raw = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        let addr = Address::from(raw);

        assert_eq!(raw.to_vec(), addr.to_vec());
    }

    #[test]
    fn should_return_ref() {
        let raw = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        let addr = Address::from(raw);

        assert_eq!(&raw, addr.as_ref());
    }

    #[test]
    fn should_have_display_impl() {
        let raw: Vec<u8> = "60f0dc62f0fac30a5beee9ac998590026923aa79"
            .from_hex()
            .unwrap();
        let addr = Address::from(raw.as_ref());

        assert_eq!(
            format!("{}", addr),
            "0x60f0dc62f0fac30a5beee9ac998590026923aa79"
        );
    }

    #[test]
    fn should_have_debug_impl() {
        let raw: Vec<u8> = "60f0dc62f0fac30a5beee9ac998590026923aa79"
            .from_hex()
            .unwrap();
        let addr = Address::from(raw.as_ref());

        assert_eq!(
            format!("{:?}", addr),
            "Address(0x60f0dc62f0fac30a5beee9ac998590026923aa79)"
        );
    }
}
