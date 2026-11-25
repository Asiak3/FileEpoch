//! Password storage in the operating system's credential store (Windows Credential Manager,
//! macOS Keychain, Secret Service on Linux). Passwords are never written anywhere else.

use zeroize::Zeroizing;

use crate::{Error, Result, ServerAddress};

const SERVICE: &str = "Rewind (Kopia)";

/// A password that is wiped from memory when dropped.
pub type Secret = Zeroizing<String>;

fn entry(server: &ServerAddress, user: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("{user}@{}", server.authority()))
        .map_err(|e| Error::Credentials(e.to_string()))
}

/// Stored password for `user` on `server`, if any.
pub fn load(server: &ServerAddress, user: &str) -> Result<Option<Secret>> {
    match entry(server, user)?.get_password() {
        Ok(p) => Ok(Some(Zeroizing::new(p))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::Credentials(e.to_string())),
    }
}

/// Stores (or replaces) the password for `user` on `server`.
pub fn store(server: &ServerAddress, user: &str, password: &str) -> Result<()> {
    entry(server, user)?.set_password(password).map_err(|e| Error::Credentials(e.to_string()))
}

/// Deletes the stored password. Returns `false` if there was none.
pub fn delete(server: &ServerAddress, user: &str) -> Result<bool> {
    match entry(server, user)?.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(Error::Credentials(e.to_string())),
    }
}
