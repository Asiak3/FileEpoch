//! `rewind-cli`: command-line test tool for the Kopia client library.
//!
//! Connection settings can be given as options or as environment variables
//! (`REWIND_SERVER`, `REWIND_USER`, `REWIND_FINGERPRINT`). The password is never accepted on the
//! command line or from the environment: it is read from the system credential store or asked
//! interactively.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Local, Utc};
use clap::{Parser, Subcommand};
use kopia_client::credentials::Secret;
use kopia_client::restore::{Naming, restore_file};
use kopia_client::{
    EntryType, Error, Fingerprint, KopiaClient, ServerAddress, Snapshot, SourceInfo, Warning, credentials,
    probe_certificate,
};

#[derive(Parser)]
#[command(name = "rewind-cli", version, about = "Read-only command-line client for a Kopia web server")]
struct Cli {
    /// Server address, e.g. https://backup.example:51515
    #[arg(long, env = "REWIND_SERVER", global = true)]
    server: Option<String>,

    /// Kopia web UI user (the server's --server-username)
    #[arg(long, env = "REWIND_USER", global = true)]
    user: Option<String>,

    /// SHA-256 fingerprint of the server certificate (see the `fingerprint` command)
    #[arg(long, env = "REWIND_FINGERPRINT", global = true)]
    fingerprint: Option<String>,

    /// After a successful login, save the password in the system credential store
    #[arg(long, global = true)]
    save_password: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the fingerprint of the server's TLS certificate (no login, no HTTP request)
    Fingerprint,
    /// Log in and show the server version and repository status
    Status,
    /// List the snapshot sources
    Sources,
    /// List the snapshots of a source, in local time
    Snapshots {
        /// Source path, its last component (e.g. `docs`) or part of it
        #[arg(long)]
        source: String,
        /// Show every snapshot instead of grouping identical ones
        #[arg(long)]
        all: bool,
    },
    /// List a directory inside a snapshot
    Ls {
        #[arg(long)]
        source: String,
        /// `latest` (default), an ID prefix, `YYYY-MM-DD HH:MM` or `YYYY-MM-DD` (local time)
        #[arg(long, default_value = "latest")]
        snapshot: String,
        /// Path inside the snapshot, components separated by `/` (default: root)
        #[arg(long, default_value = "")]
        path: String,
    },
    /// Download one file from a snapshot into a local folder (never overwrites)
    Download {
        #[arg(long)]
        source: String,
        #[arg(long, default_value = "latest")]
        snapshot: String,
        /// Path of the file inside the snapshot, components separated by `/`
        #[arg(long)]
        path: String,
        /// Local folder to write into
        #[arg(long)]
        out: PathBuf,
        /// Keep the original name instead of adding "(restored <snapshot time>)"
        #[arg(long)]
        keep_name: bool,
    },
    /// Delete the password saved for --server/--user
    ForgetPassword,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {e:#}");
        if let Some(Error::CertificateMismatch { .. }) = e.downcast_ref::<Error>() {
            eprintln!("hint: run `rewind-cli fingerprint` and compare with what the server prints at startup.");
        }
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let server: ServerAddress =
        cli.server.as_deref().ok_or_else(|| anyhow!("--server (or REWIND_SERVER) is required"))?.parse()?;

    match &cli.command {
        Command::Fingerprint => {
            let fp = probe_certificate(&server).await?;
            println!("Server certificate SHA-256 (end-entity):");
            println!("  {}", fp.to_hex());
            println!("  {}", fp.to_colon_hex());
            println!("Compare it with `SERVER CERT SHA256` in the Kopia server log before trusting it.");
            return Ok(());
        }
        Command::ForgetPassword => {
            let user = cli.user.as_deref().ok_or_else(|| anyhow!("--user (or REWIND_USER) is required"))?;
            let deleted = credentials::delete(&server, user)?;
            println!("{}", if deleted { "Saved password deleted." } else { "No saved password." });
            return Ok(());
        }
        _ => {}
    }

    let client = login(&cli, &server).await?;

    match cli.command {
        Command::Fingerprint | Command::ForgetPassword => unreachable!(),
        Command::Status => {
            let st = client.repo_status().await?;
            println!("Server:      {server}");
            println!("Kopia:       v{}", client.server_version().unwrap_or_else(|| "?".into()));
            println!("Read-only:   {}", if st.readonly { "yes" } else { "NO" });
            if !st.description.is_empty() {
                println!("Repository:  {}", st.description);
            }
        }
        Command::Sources => {
            let resp = client.sources().await?;
            for s in &resp.sources {
                println!("{}", s.source);
            }
        }
        Command::Snapshots { source, all } => {
            let src = find_source(&client, &source).await?;
            let resp = client.snapshots(&src, all).await?;
            println!("{src}");
            println!(
                "{} snapshot(s) shown ({} in total, {} with different content){}",
                resp.snapshots.len(),
                resp.unfiltered_count,
                resp.unique_count,
                if all { "" } else { "; identical ones grouped, use --all to see each" }
            );
            println!("{:<17}  {:<8}  {:>9}  {:>10}  Retention", "Start (local)", "ID", "Files", "Size");
            for s in resp.snapshots.iter().rev() {
                let (files, size) = s.summary.as_ref().map(|x| (x.files, x.size)).unwrap_or_default();
                let mut notes = s.retention.join(",");
                if !s.pins.is_empty() {
                    notes.push_str(&format!(" pinned:{}", s.pins.join(",")));
                }
                if let Some(r) = &s.incomplete {
                    notes.push_str(&format!(" INCOMPLETE({r})"));
                }
                println!(
                    "{:<17}  {:<8}  {:>9}  {:>10}  {}",
                    local(s.start_time),
                    short_id(&s.id),
                    files,
                    human_size(size),
                    notes
                );
            }
        }
        Command::Ls { source, snapshot, path } => {
            let src = find_source(&client, &source).await?;
            let snap = select_snapshot(&client, &src, &snapshot).await?;
            let dir = if path.split('/').all(|p| p.is_empty()) {
                snap.root_id.clone()
            } else {
                let e = client.find_entry(&snap.root_id, &path).await?;
                if e.entry_type != EntryType::Directory {
                    bail!("{path} is not a directory");
                }
                e.obj
            };
            let mut entries = client.list_directory(&dir).await?.entries.clone();
            entries.sort_by(|a, b| {
                (a.entry_type != EntryType::Directory, a.name.to_lowercase())
                    .cmp(&(b.entry_type != EntryType::Directory, b.name.to_lowercase()))
            });
            println!(
                "{src}  snapshot {} ({})  /{}",
                local(snap.start_time),
                short_id(&snap.id),
                path.trim_matches('/')
            );
            for e in &entries {
                let (kind, size) = match e.entry_type {
                    EntryType::Directory => ("dir ", e.summ.as_ref().map(|s| s.size).unwrap_or(0)),
                    EntryType::File => ("file", e.size),
                    EntryType::Symlink => ("link", e.size),
                    EntryType::Unknown => ("?   ", e.size),
                };
                let mtime = e.mtime.map(local).unwrap_or_default();
                println!("{kind}  {:>10}  {:<17}  {}", human_size(size), mtime, e.name);
            }
            println!("{} entries", entries.len());
        }
        Command::Download { source, snapshot, path, out, keep_name } => {
            let src = find_source(&client, &source).await?;
            let snap = select_snapshot(&client, &src, &snapshot).await?;
            let entry = client.find_entry(&snap.root_id, &path).await?;
            match entry.entry_type {
                EntryType::File => {}
                EntryType::Directory => bail!("{path} is a folder; folder restore comes in phase 3"),
                EntryType::Symlink => bail!("{path} is a symbolic link; links are not restored"),
                EntryType::Unknown => bail!("{path} has an unknown type"),
            }
            if !out.is_dir() {
                bail!("output folder {} does not exist", out.display());
            }
            let naming = if keep_name {
                Naming::Original
            } else {
                Naming::Suffix { label: "restored".into(), time: snap.start_time.with_timezone(&Local) }
            };
            println!(
                "Downloading {} ({}) from snapshot {} ...",
                entry.name,
                human_size(entry.size),
                local(snap.start_time)
            );
            let started = Instant::now();
            let done = restore_file(&client, &entry, &out, &naming).await?;
            let secs = started.elapsed().as_secs_f64().max(0.001);
            println!(
                "Saved {} ({} bytes, {}/s)",
                done.path.display(),
                done.bytes,
                human_size((done.bytes as f64 / secs) as u64)
            );
            if done.sanitized {
                println!(
                    "Note: the name contained characters not allowed on this system; they were replaced with '_'."
                );
            }
            if let Some(m) = entry.mtime {
                println!("Modification time set to {}", local(m));
            }
        }
    }
    Ok(())
}

/// Logs in with the saved password or by asking for it, and prints connection warnings.
async fn login(cli: &Cli, server: &ServerAddress) -> Result<KopiaClient> {
    let user = cli.user.as_deref().ok_or_else(|| anyhow!("--user (or REWIND_USER) is required"))?;
    let fingerprint: Fingerprint = cli
        .fingerprint
        .as_deref()
        .ok_or_else(|| {
            anyhow!("--fingerprint (or REWIND_FINGERPRINT) is required; get it with `rewind-cli fingerprint`")
        })?
        .parse()?;

    // Check the certificate before asking for (or sending) a password.
    let presented = probe_certificate(server).await?;
    if presented != fingerprint {
        return Err(Error::CertificateMismatch { presented: presented.to_hex() }.into());
    }

    let saved = match credentials::load(server, user) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("warning: cannot read the credential store: {e}");
            None
        }
    };
    let mut from_store = saved.is_some();
    let mut password = match saved {
        Some(p) => p,
        None => ask_password(user, server)?,
    };

    loop {
        let client = KopiaClient::new(server.clone(), user, &password, fingerprint)?;
        match client.connect().await {
            Ok(info) => {
                for w in &info.warnings {
                    match w {
                        Warning::UntestedVersion(v) => eprintln!(
                            "warning: Kopia server version {} has not been tested with this tool (tested: {})",
                            v.as_deref().unwrap_or("unknown"),
                            kopia_client::TESTED_VERSIONS.join(", ")
                        ),
                        Warning::NotReadOnly => eprintln!(
                            "WARNING: this server's repository is NOT read-only. This tool only reads, but check \
                             that you are not connected to an administration server."
                        ),
                    }
                }
                if cli.save_password && !from_store {
                    credentials::store(server, user, &password).context("saving the password")?;
                    eprintln!("Password saved in the system credential store.");
                }
                return Ok(client);
            }
            Err(Error::AccessDenied) if from_store => {
                eprintln!("The saved password was rejected.");
                from_store = false;
                password = ask_password(user, server)?;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn ask_password(user: &str, server: &ServerAddress) -> Result<Secret> {
    let p = rpassword::prompt_password(format!("Password for {user} at {server}: ")).context("reading the password")?;
    Ok(Secret::new(p))
}

/// Finds a source by exact path, then last path component, then substring.
/// Fails if a tier matches several sources.
async fn find_source(client: &KopiaClient, query: &str) -> Result<SourceInfo> {
    let sources: Vec<SourceInfo> = client.sources().await?.sources.into_iter().map(|s| s.source).collect();
    let tiers: [&dyn Fn(&SourceInfo) -> bool; 4] =
        [&|s| s.to_string() == query, &|s| s.path == query, &|s| s.path.rsplit('/').next() == Some(query), &|s| {
            s.path.contains(query)
        }];
    for tier in tiers {
        let found: Vec<&SourceInfo> = sources.iter().filter(|s| tier(s)).collect();
        match found.len() {
            0 => continue,
            1 => return Ok(found[0].clone()),
            _ => {
                let list: Vec<String> = found.iter().map(|s| format!("  {s}")).collect();
                bail!("'{query}' matches several sources; be more specific:\n{}", list.join("\n"));
            }
        }
    }
    let list: Vec<String> = sources.iter().map(|s| format!("  {s}")).collect();
    bail!("no source matches '{query}'. Available:\n{}", list.join("\n"))
}

async fn select_snapshot(client: &KopiaClient, src: &SourceInfo, sel: &str) -> Result<Snapshot> {
    let mut snaps = client.snapshots(src, true).await?.snapshots;
    if snaps.is_empty() {
        bail!("{src} has no snapshots");
    }
    if sel == "latest" {
        return Ok(snaps.pop().expect("not empty"));
    }
    let matches: Vec<Snapshot> = snaps
        .into_iter()
        .filter(|s| {
            let t = s.start_time.with_timezone(&Local);
            s.id.starts_with(sel)
                || t.format("%Y-%m-%d %H:%M").to_string() == sel
                || t.format("%Y-%m-%d").to_string() == sel
        })
        .collect();
    // For a date, the newest snapshot of that day is the natural choice.
    matches
        .into_iter()
        .last()
        .ok_or_else(|| anyhow!("no snapshot matches '{sel}' (use `snapshots --all` to list them)"))
}

fn local(t: DateTime<Utc>) -> String {
    t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
}

fn short_id(id: &str) -> &str {
    &id[..id.len().min(8)]
}

fn human_size(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u + 1 < UNITS.len() {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{n} B") } else { format!("{v:.1} {}", UNITS[u]) }
}
