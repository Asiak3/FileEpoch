//! Password storage in the operating system's credential store (Windows Credential Manager,
//! macOS Keychain, Secret Service on Linux). Passwords are never written anywhere else.

use zeroize::Zeroizing;

use crate::{Error, Result, ServerAddress};

const SERVICE: &str = "FileEpoch (Kopia)";
/// Names used by earlier versions (the app was called "Rewind"). A password found there is moved
/// to [`SERVICE`] the first time it is read.
const LEGACY_SERVICES: &[&str] = &["Rewind (Kopia)"];

/// A password that is wiped from memory when dropped.
pub type Secret = Zeroizing<String>;

fn entry_in(service: &str, server: &ServerAddress, user: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(service, &format!("{user}@{}", server.authority()))
        .map_err(|e| Error::Credentials(e.to_string()))
}

fn entry(server: &ServerAddress, user: &str) -> Result<keyring::Entry> {
    entry_in(SERVICE, server, user)
}

fn read(entry: &keyring::Entry) -> Result<Option<Secret>> {
    match entry.get_password() {
        Ok(p) => Ok(Some(Zeroizing::new(p))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::Credentials(e.to_string())),
    }
}

/// Stored password for `user` on `server`, if any.
pub fn load(server: &ServerAddress, user: &str) -> Result<Option<Secret>> {
    if let Some(p) = read(&entry(server, user)?)? {
        return Ok(Some(p));
    }
    for legacy in LEGACY_SERVICES {
        let old = entry_in(legacy, server, user)?;
        if let Some(p) = read(&old)? {
            // Move it: store under the current name first, then remove the old entry.
            store(server, user, &p)?;
            let _ = old.delete_credential();
            return Ok(Some(p));
        }
    }
    Ok(None)
}

/// Stores (or replaces) the password for `user` on `server`.
pub fn store(server: &ServerAddress, user: &str, password: &str) -> Result<()> {
    entry(server, user)?.set_password(password).map_err(|e| Error::Credentials(e.to_string()))
}

/// Deletes the stored password. Returns `false` if there was none.
pub fn delete(server: &ServerAddress, user: &str) -> Result<bool> {
    let mut found = false;
    for service in std::iter::once(&SERVICE).chain(LEGACY_SERVICES) {
        match entry_in(service, server, user)?.delete_credential() {
            Ok(()) => found = true,
            Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(Error::Credentials(e.to_string())),
        }
    }
    Ok(found)
}
