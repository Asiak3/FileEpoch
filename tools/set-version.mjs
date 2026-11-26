// Sets the FileEpoch version everywhere it is written, or checks that they agree.
//
//   node tools/set-version.mjs 0.2.0     # set
//   node tools/set-version.mjs --check   # exit 1 if the versions differ (used by CI)
//
// Sources: app/package.json (read by tauri.conf.json), the [workspace.package] version in
// Cargo.toml (all crates) and Cargo.lock. Run from the repository root.

import { readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

const PKG = "app/package.json";
const LOCK = "app/package-lock.json";
const CARGO = "Cargo.toml";
const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;
const workspaceVersion = /(\[workspace\.package\][^[]*?\nversion\s*=\s*")([^"]+)(")/;

function versions() {
  const pkg = JSON.parse(readFileSync(PKG, "utf8")).version;
  const cargo = readFileSync(CARGO, "utf8").match(workspaceVersion)?.[2];
  return { pkg, cargo };
}

const arg = process.argv[2];
if (arg === "--check" || !arg) {
  const { pkg, cargo } = versions();
  if (pkg !== cargo) {
    console.error(`Version mismatch: ${PKG} has ${pkg}, ${CARGO} has ${cargo}. Run: node tools/set-version.mjs <version>`);
    process.exit(1);
  }
  console.log(`Version ${pkg}`);
  process.exit(0);
}

if (!SEMVER.test(arg)) {
  console.error(`Not a version: ${arg} (expected e.g. 1.2.3 or 1.2.3-beta.1)`);
  process.exit(1);
}

for (const file of [PKG, LOCK]) {
  const json = JSON.parse(readFileSync(file, "utf8"));
  json.version = arg;
  if (json.packages?.[""]) json.packages[""].version = arg;
  writeFileSync(file, JSON.stringify(json, null, 2) + "\n");
}
const cargo = readFileSync(CARGO, "utf8");
if (!workspaceVersion.test(cargo)) {
  console.error(`No [workspace.package] version in ${CARGO}`);
  process.exit(1);
}
writeFileSync(CARGO, cargo.replace(workspaceVersion, `$1${arg}$3`));
// Refresh the crate versions recorded in Cargo.lock (no dependency changes).
execFileSync("cargo", ["update", "--workspace", "--offline"], { stdio: "inherit" });
console.log(`Version set to ${arg}`);
