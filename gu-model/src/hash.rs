use digest::generic_array::GenericArray;
use digest::{Digest, DynDigest};
use failure::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "bad hash val. expected {}, was {}", _0, _1)]
    BadSize(usize, usize),
    #[fail(display = "invalid char in hash val. {}", _0)]
    BadChar(u8),
    #[fail(display = "unrecognized hash format")]
    InvalidHashFormat,

    #[fail(display = "invalid hash type: {}/{}", _0, _1)]
    UnknownHashFunc(String, usize),
}

pub trait ContentChecker {
    fn update(&mut self, chunk: &[u8]);

    fn verify(self) -> bool;
}

pub trait DynContentChecker {
    fn update_box(&mut self, chunk: &[u8]);

    fn verify_box(self: Box<Self>) -> bool;
}

impl<D: ContentChecker + Clone> DynContentChecker for D {
    fn update_box(&mut self, chunk: &[u8]) {
        ContentChecker::update(self, chunk);
    }

    fn verify_box(self: Box<Self>) -> bool {
        ContentChecker::verify((*self).clone())
    }
}

impl ContentChecker for Box<dyn DynContentChecker> {
    fn update(&mut self, chunk: &[u8]) {
        DynContentChecker::update_box(&mut **self, chunk)
    }

    fn verify(self) -> bool {
        DynContentChecker::verify_box(self)
    }
}

#[derive(Clone)]
struct DigestContentChecker<D: Digest + Clone> {
    digest: D,
    expected_hash: GenericArray<u8, D::OutputSize>,
}

fn to_hex(ch: u8) -> Result<u8, Error> {
    match ch {
        b'0'..=b'9' => Ok(ch - b'0'),
        b'a'..=b'f' => Ok(ch - b'a' + 10),
        b'A'..=b'F' => Ok(ch - b'a' + 10),
        _ => Err(Error::BadChar(ch)),
    }
}

impl<D: Digest + Clone> DigestContentChecker<D> {
    pub fn from_hexstr(digest: D, hexstr: &[u8]) -> Result<Self, Error> {
        let output_size = D::output_size();
        if hexstr.len() != output_size * 2 {
            return Err(Error::BadSize(output_size * 2, hexstr.len()));
        }

        let expected_hash = hexstr
            .chunks_exact(2)
            .map(|ch| Ok(to_hex(ch[0])? << 4 | to_hex(ch[1])?))
            .collect::<Result<_, Error>>()?;

        Ok(DigestContentChecker {
            digest,
            expected_hash,
        })
    }
}

impl<D: Digest + Clone> ContentChecker for DigestContentChecker<D> {
    fn update(&mut self, chunk: &[u8]) {
        self.digest.input(chunk)
    }

    fn verify(self) -> bool {
        self.digest.result() == self.expected_hash
    }
}

pub struct ParsedHash<'a> {
    hash_name: &'a [u8],
    hash_value: &'a [u8],
}

impl<'a> ParsedHash<'a> {
    pub fn from_hash_bytes(s: &'a [u8]) -> Result<Self, Error> {
        if let Some(p) = s.iter().position(|&ch| ch == b':') {
            let (hash_name, hv) = s.split_at(p);
            let hash_value = &hv[1..];
            Ok(ParsedHash {
                hash_name,
                hash_value,
            })
        } else {
            Err(Error::InvalidHashFormat)
        }
    }

    pub fn algo_name(&self) -> Result<&str, Error> {
        ::std::str::from_utf8(self.hash_name).map_err(|_| Error::InvalidHashFormat)
    }

    pub fn to_hash_str(&self) -> Result<String, Error> {
        Ok(format!("{}:{}", self.algo_name()?, self.value()?))
    }

    pub fn to_path(&self) -> Result<PathBuf, Error> {
        Ok(format!("{}---{}", self.algo_name()?, self.value()?).into())
    }

    pub fn from_file_name<AP: AsRef<Path> + ?Sized>(s: &'a AP) -> Result<Self, Error> {
        s.as_ref()
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::InvalidHashFormat)
            .and_then(|s| {
                let mut it = s.split("---").fuse();
                match (it.next(), it.next(), it.next()) {
                    (Some(hash_name), Some(hash_value), None) => Ok(ParsedHash {
                        hash_name: hash_name.as_ref(),
                        hash_value: hash_value.as_ref(),
                    }),
                    _ => Err(Error::InvalidHashFormat),
                }
            })
    }

    pub fn digest(&self) -> Result<Box<dyn DynDigest>, Error> {
        digest(self.hash_name, self.hash_value.len() * 4)
    }

    pub fn checker(&self) -> Result<Box<dyn DynContentChecker>, Error> {
        Ok(match (self.hash_name, self.hash_value.len() * 4) {
            (b"SHA3", 224) => Box::new(self.value_checker::<sha3::Sha3_224>()?),
            (b"SHA3", 256) => Box::new(self.value_checker::<sha3::Sha3_256>()?),
            (b"SHA3", 384) => Box::new(self.value_checker::<sha3::Sha3_384>()?),
            (b"SHA3", 512) => Box::new(self.value_checker::<sha3::Sha3_512>()?),
            (b"SHA1", 160) => Box::new(self.value_checker::<sha1::Sha1>()?),
            _ => return Err(Error::InvalidHashFormat),
        })
    }

    #[inline]
    pub fn value_bytes(&self) -> &'a [u8] {
        self.hash_value
    }

    pub fn value(&self) -> Result<&str, Error> {
        ::std::str::from_utf8(self.hash_value).map_err(|_| Error::InvalidHashFormat)
    }

    #[inline]
    fn value_checker<D: Digest + Default + Clone>(&self) -> Result<DigestContentChecker<D>, Error> {
        DigestContentChecker::from_hexstr(D::default(), self.hash_value)
    }
}

pub fn digest<R: AsRef<[u8]>>(hash_name: R, bits: usize) -> Result<Box<dyn DynDigest>, Error> {
    Ok(match (hash_name.as_ref(), bits) {
        (b"SHA3", 224) => Box::new(sha3::Sha3_224::default()),
        (b"SHA3", 256) => Box::new(sha3::Sha3_256::default()),
        (b"SHA3", 384) => Box::new(sha3::Sha3_384::default()),
        (b"SHA3", 512) => Box::new(sha3::Sha3_512::default()),
        (b"SHA1", 160) => Box::new(sha1::Sha1::default()),
        _ => {
            return Err(Error::UnknownHashFunc(
                String::from_utf8(hash_name.as_ref().to_vec()).unwrap_or_else(|_| "invalid".into()),
                bits,
            ));
        }
    })
}

pub fn checker<R: AsRef<[u8]>>(hash_str: R) -> Result<impl ContentChecker, Error> {
    ParsedHash::from_hash_bytes(hash_str.as_ref())?.checker()
}

#[cfg(test)]
mod test {
    use super::*;
    use sha3::Sha3_224;

    #[test]
    fn test_create_digest() {
        let d = Sha3_224::default();
        assert!(DigestContentChecker::from_hexstr(
            d,
            b"f66cb3691983cbd212fc06177d7eb0baecf36d2a0362c9b36d9c2306"
        )
        .is_ok());
        assert!(DigestContentChecker::from_hexstr(
            Sha3_224::default(),
            b"f66cb3691983cbd212fc06177d7eb0baecf36d2a0362c9b36d9c230"
        )
        .is_err());
        let mut checker = DigestContentChecker::from_hexstr(
            Sha3_224::default(),
            b"550a8e7b4e6a1bdeb998fb3f03908d1aba5ad3556c197be719f41fe2",
        )
        .unwrap();
        checker.update(b"golem");
        assert_eq!(checker.verify(), true);
        ()
    }

    fn test_value(content: &[u8], hash_str: &str) {
        let mut d = checker(hash_str).unwrap();
        d.update(content);
        assert!(d.verify())
    }

    #[test]
    fn test_gen_checker() {
        test_value(
            b"golem1",
            "SHA3:dd1a350cfe1d851f36a40d2b0f9f705a0bc076ab31dd81a662ebdf40",
        );
        test_value(
            b"golem2",
            "SHA3:7ba62e92095980b4fd8a743d608d8a5b0b0224105ddab845845b7c622c60f248",
        );
        test_value(b"golem2", "SHA3:9fa5c15b117a49c638aa438e2b6e33601360732e8d1f776535d93e21f733dd501c9756fa2feb508d3daf180253ecc1ef");
        test_value(b"golem1", "SHA3:e43d55ac264ee607918a78561e1f45779b192c747f5844d08a63697314ccf2445edb823cd6bbe14782a40a932176bcda9f35c097cbf49872095205ad102a7960")
    }

    #[test]
    fn test_parser() {
        let s =
            ParsedHash::from_hash_bytes(b"SHA1:c04e69c52dc35d93389a23189c333d150cadd719").unwrap();

        eprintln!("{:?}", ::std::str::from_utf8(s.value_bytes()));

        let mut digest = s.digest().unwrap();
        digest.input(b"alamakota");
        let _ = digest.result();

        let s =
            ParsedHash::from_file_name("SHA1---c04e69c52dc35d93389a23189c333d150cadd719").unwrap();
        let mut c = s.checker().unwrap();
        c.update(b"alamakota\n");
        assert!(c.verify());

        assert_eq!(
            s.to_hash_str().unwrap(),
            "SHA1:c04e69c52dc35d93389a23189c333d150cadd719"
        );
        assert_eq!(
            s.to_path().unwrap(),
            PathBuf::from("SHA1---c04e69c52dc35d93389a23189c333d150cadd719")
        );
    }
}
