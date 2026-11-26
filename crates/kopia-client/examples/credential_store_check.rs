//! Checks that the system credential store really persists passwords across processes
//! (and is not an in-memory mock). Uses a dummy server and secret.
//!
//! ```text
//! cargo run -p kopia-client --example credential_store_check -- set
//! cargo run -p kopia-client --example credential_store_check -- get
//! cargo run -p kopia-client --example credential_store_check -- delete
//! ```

use kopia_client::{ServerAddress, credentials};

const DUMMY_SECRET: &str = "fileepoch-credential-store-check";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server: ServerAddress = "https://credential-check.invalid:1".parse()?;
    let user = "fileepoch-check";
    match std::env::args().nth(1).as_deref() {
        Some("set") => {
            credentials::store(&server, user, DUMMY_SECRET)?;
            println!("stored");
        }
        Some("get") => match credentials::load(&server, user)? {
            Some(p) if p.as_str() == DUMMY_SECRET => println!("found (persisted across processes)"),
            Some(_) => println!("found, but with a different value"),
            None => println!("NOT found"),
        },
        Some("delete") => println!("deleted: {}", credentials::delete(&server, user)?),
        _ => eprintln!("usage: credential_store_check set|get|delete"),
    }
    Ok(())
}
