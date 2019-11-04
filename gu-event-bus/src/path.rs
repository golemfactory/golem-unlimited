use std::str;

pub struct EventPath {
    path_chars: String,
}

impl EventPath {
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        let path = self.path_chars.as_ref();
        let offset = 0;

        EventPathParts { path, offset }
    }
}

impl<'a> From<&'a str> for EventPath {
    fn from(p: &'a str) -> Self {
        EventPath {
            path_chars: p.into(),
        }
    }
}

struct EventPathParts<'a> {
    path: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for EventPathParts<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        let len = self.path.len();
        if self.offset >= len {
            return None;
        }

        while self.offset < len {
            if self.path[self.offset] == b'/' {
                let end = self.offset;
                self.offset += 1;
                return Some(unsafe { str::from_utf8_unchecked(&self.path[0..end]) });
            }
            self.offset += 1;
        }
        return Some(unsafe { str::from_utf8_unchecked(&self.path) });
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_path() {
        let p1: EventPath = "ala/ma/kota".into();

        let pl: Vec<&str> = p1.iter().collect();
        assert_eq!(pl[0], "ala");
        assert_eq!(pl[1], "ala/ma");
        assert_eq!(pl[2], "ala/ma/kota");
        assert_eq!(pl.len(), 3);
    }

    #[test]
    fn test_path2() {
        let p1: EventPath = "/plugins/Mining".into();

        let pl: Vec<&str> = p1.iter().collect();
        assert_eq!(pl[0], "");
        assert_eq!(pl[1], "/plugins");
        assert_eq!(pl[2], "/plugins/Mining");
        assert_eq!(pl.len(), 3);

        for it in p1.iter() {
            println!("it={}", it)
        }
    }
}
