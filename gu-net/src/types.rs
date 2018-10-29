use std::{
    fmt,
    str::{self, FromStr},
};

use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Cow, mem::uninitialized};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct NodeId {
    inner: [u8; 20],
}

impl NodeId {
    #[inline(always)]
    fn with_hex<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&str) -> R,
    {
        let mut hex_str: [u8; 42] = unsafe { uninitialized() };

        hex_str[0] = '0' as u8;
        hex_str[1] = 'x' as u8;

        let mut ptr = 2;
        for it in &self.inner {
            let hi = (it >> 4) & 0xfu8;
            let lo = it & 0xf;
            hex_str[ptr] = HEX_CHARS[hi as usize];
            hex_str[ptr + 1] = HEX_CHARS[lo as usize];
            ptr += 2;
        }
        assert_eq!(ptr, hex_str.len());

        let hex_str = unsafe { str::from_utf8_unchecked(&hex_str) };
        f(hex_str)
    }
}

impl Default for NodeId {
    fn default() -> Self {
        NodeId { inner: [0; 20] }
    }
}

impl AsRef<[u8]> for NodeId {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

impl Distribution<NodeId> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &'_ mut R) -> NodeId {
        NodeId { inner: rng.gen() }
    }
}

impl From<[u8; 20]> for NodeId {
    fn from(inner: [u8; 20]) -> Self {
        NodeId { inner }
    }
}

impl<'a> From<&'a [u8]> for NodeId {
    fn from(it: &'a [u8]) -> Self {
        let mut inner = [0; 20];
        inner.copy_from_slice(it);

        NodeId { inner }
    }
}

impl<'a> From<Cow<'a, [u8]>> for NodeId {
    fn from(it: Cow<'a, [u8]>) -> Self {
        it.as_ref().into()
    }
}

impl ToString for NodeId {
    fn to_string(&self) -> String {
        self.with_hex(|str| str.into())
    }
}

#[inline]
fn hex_to_dec(hex: u8) -> Result<u8, ()> {
    match hex {
        b'A'...b'F' => Ok(hex - b'A' + 10),
        b'a'...b'f' => Ok(hex - b'a' + 10),
        b'0'...b'9' => Ok(hex - b'0'),
        _ => Err(()),
    }
}

impl str::FromStr for NodeId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        let bytes = s.as_bytes();

        if bytes.len() != 42 {
            return Err(());
        }

        if bytes[0] != b'0' || bytes[1] != b'x' {
            return Err(());
        }

        let mut inner = [0u8; 20];
        let mut p = 0;

        for b in bytes[2..].chunks(2) {
            let (hi, lo) = (hex_to_dec(b[0])?, hex_to_dec(b[1])?);
            inner[p] = (hi << 4) | lo;
            p += 1;
        }
        assert_eq!(p, 20);

        Ok(NodeId { inner })
    }
}

static HEX_CHARS: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'a', b'b', b'c', b'd', b'e', b'f',
];

impl Serialize for NodeId {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        self.with_hex(|hex_str| serializer.serialize_str(hex_str))
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.with_hex(|hex_str| write!(f, "{}", hex_str))
    }
}

struct NodeIdVisit;

impl<'de> de::Visitor<'de> for NodeIdVisit {
    type Value = NodeId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a nodeId")
    }

    fn visit_str<E>(self, v: &str) -> Result<<Self as de::Visitor<'de>>::Value, E>
    where
        E: de::Error,
    {
        match NodeId::from_str(v) {
            Ok(node_id) => Ok(node_id),
            Err(_) => Err(de::Error::custom("invalid format")),
        }
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<<Self as de::Visitor<'de>>::Value, E>
    where
        E: de::Error,
    {
        if v.len() == 20 {
            let mut inner: [u8; 20] = unsafe { uninitialized() };
            inner.copy_from_slice(v);
            Ok(NodeId { inner })
        } else {
            Err(de::Error::custom("invalid format"))
        }
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(NodeIdVisit)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use rand::*;
    use serde_json;

    #[test]
    fn test_hex() {
        let node_id: NodeId = thread_rng().gen();

        println!("json={}", serde_json::to_string_pretty(&node_id).unwrap());

        let str = format!("{:?}", node_id);
        assert_eq!(node_id, NodeId::from_str(&str).unwrap());
    }

}
