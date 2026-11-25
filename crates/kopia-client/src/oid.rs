use std::fmt;
use std::str::FromStr;

use serde::Deserialize;

use crate::Error;

/// A validated Kopia object ID.
///
/// Grammar (from `repo/object/objectid.go` and `repo/content/index/id.go`, Kopia v0.23.1):
///
/// ```text
/// OID     = "I"* ["Z"] CONTENT      ("I" and "Z" are mutually exclusive)
/// CONTENT = [prefix g..z] HEX       (lower-case hex, even length, 2..=64 characters)
/// ```
///
/// Validation guarantees the ID can be placed in a URL path without escaping, so no `/`, `?`,
/// `%` or `..` can ever reach the request path. Whether an object is a file or a directory must be
/// decided from the directory entry's type, never from the ID.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "String")]
pub struct Oid(String);

impl Oid {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Oid {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i] == b'I' {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'Z' {
            if i > 0 {
                return Err(Error::InvalidOid(s.to_owned()));
            }
            i += 1;
        }
        if i < bytes.len() && (b'g'..=b'z').contains(&bytes[i]) {
            i += 1;
        }
        let hex = &bytes[i..];
        let valid_hex = (2..=64).contains(&hex.len())
            && hex.len().is_multiple_of(2)
            && hex.iter().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c));
        if valid_hex { Ok(Oid(s.to_owned())) } else { Err(Error::InvalidOid(s.to_owned())) }
    }
}

impl TryFrom<String> for Oid {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Oid({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_documented_grammar() {
        for ok in [
            "8f3e",                                                             // plain file content
            "k0123456789abcdef0123456789abcdef",                                // directory
            "Ik0123abcd",                                                       // chunked directory
            "IIk0123abcd",                                                      // double indirection
            "I0123abcd",                                                        // chunked file
            "Z0123abcd",                                                        // compressed
            "Zk0123abcd",                                                       // compressed directory
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", // 64 hex
        ] {
            assert!(ok.parse::<Oid>().is_ok(), "{ok} should be valid");
        }
    }

    #[test]
    fn rejects_everything_else() {
        for bad in [
            "",
            "I",
            "Z",
            "k",
            "abc",
            "IZk01",
            "ZI01",
            "K0123",
            "a0123",
            "0123ABCD",
            "k01/23",
            "../repo",
            "k01?x=1",
            "k01%2f",
            "k01 23",
            "k0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef00",
        ] {
            assert!(bad.parse::<Oid>().is_err(), "{bad:?} should be rejected");
        }
    }
}
