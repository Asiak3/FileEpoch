// Prints the text of a GitHub release: the CHANGELOG.md entry of a version plus the download table.
// The Release workflow uses it, so a draft already has its notes when it is created.
//
//   node tools/release-notes.mjs [version]     (default: the version in app/package.json)
//
// Fails if CHANGELOG.md has no entry for the version, so a release cannot go out without notes.
// Run from the repository root.

import { readFileSync } from "node:fs";

const version = process.argv[2] ?? JSON.parse(readFileSync("app/package.json", "utf8")).version;
const lines = readFileSync("CHANGELOG.md", "utf8").split(/\r?\n/);

const start = lines.findIndex((l) => l.startsWith(`## [${version}]`));
if (start < 0) {
  console.error(`CHANGELOG.md has no entry for ${version}. Add one before tagging.`);
  process.exit(1);
}
let end = lines.findIndex((l, i) => i > start && l.startsWith("## ["));
if (end < 0) end = lines.length;
const entry = lines.slice(start + 1, end).join("\n").trim();
if (!entry) {
  console.error(`The CHANGELOG.md entry for ${version} is empty.`);
  process.exit(1);
}

console.log(`## What's new

${entry}

## Downloads

| System | File | Notes |
|---|---|---|
| Windows 10/11 | \`FileEpoch_${version}_x64-setup.exe\` | Installs for the current user, no administrator rights. Not code-signed yet: SmartScreen may warn ("More info" → "Run anyway"). |
| Linux | \`FileEpoch_${version}_amd64.AppImage\` or \`.deb\` | Needs WebKitGTK 4.1. Little tested so far. |
| macOS | \`FileEpoch_${version}_universal.dmg\` | Not signed by Apple yet: right-click → Open. Little tested so far. |

If you already have FileEpoch installed, it offers new versions by itself. Full list of changes in [CHANGELOG.md](https://github.com/Asiak3/FileEpoch/blob/main/CHANGELOG.md).`);
