// Generates app/public/third-party-notices.html: the license texts of everything that ships inside
// FileEpoch (Rust crates via cargo-about, npm packages that end up in the built UI), plus
// FileEpoch's own license. The page is part of the app and opened from Settings > About.
//
//   node tools/third-party-notices.mjs           # regenerate
//   node tools/third-party-notices.mjs --check   # exit 1 if the committed page is out of date (CI)
//
// Needs cargo-about (cargo install cargo-about --features cli) and the UI dependencies
// (npm ci in app/). Accepted licenses and per-crate clarifications are in about.toml.

import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const OUT = "app/public/third-party-notices.html";
const check = process.argv.includes("--check");
const tmp = mkdtempSync(join(tmpdir(), "fileepoch-notices-"));

try {
  // --- Rust crates -------------------------------------------------------------------------
  const aboutJson = join(tmp, "about.json");
  execFileSync("cargo", ["about", "generate", "--format", "json", "--fail", "-o", aboutJson], { stdio: ["ignore", "ignore", "inherit"] });
  const about = JSON.parse(readFileSync(aboutJson, "utf8"));
  const own = new Set(["fileepoch-app", "fileepoch-cli", "kopia-client"]);

  /** text → { id, name, packages: Set<string> } */
  const groups = new Map();
  const add = (id, name, text, pkg) => {
    const key = normalize(text);
    if (!groups.has(key)) groups.set(key, { id, name, text: key, packages: new Set() });
    groups.get(key).packages.add(pkg);
  };
  const mpl = new Set();

  for (const lic of about.licenses) {
    for (const used of lic.used_by) {
      const c = used.crate;
      if (own.has(c.name)) continue;
      const label = `${c.name} ${c.version} (Rust)`;
      add(lic.id, lic.name, lic.text, label);
      if (lic.id === "MPL-2.0") mpl.add(`https://crates.io/crates/${c.name}/${c.version}`);
    }
  }

  // --- npm packages in the built UI ------------------------------------------------------------
  const listFile = join(tmp, "packages.json");
  const vite = join("node_modules", "vite", "bin", "vite.js");
  execFileSync(process.execPath, [vite, "build", "--outDir", join(tmp, "dist"), "--emptyOutDir", "--logLevel", "warn"], {
    cwd: "app",
    stdio: ["ignore", "ignore", "inherit"],
    env: { ...process.env, FILEEPOCH_PACKAGES_OUT: listFile },
  });
  for (const name of JSON.parse(readFileSync(listFile, "utf8"))) {
    const dir = join("app", "node_modules", ...name.split("/"));
    const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    // LICENSE, LICENSE.md, COPYING, or one file per license of a dual license (LICENSE_MIT, …).
    const files = readdirSync(dir).filter((f) => /^(licen[cs]e|copying)([._-][\w.-]+)?$/i.test(f));
    if (!files.length) throw new Error(`${name}: no license file found`);
    for (const file of files.sort()) {
      const fromName = /mit/i.test(file) ? "MIT" : /apache/i.test(file) ? "Apache-2.0" : null;
      const id = fromName ?? (typeof pkg.license === "string" ? pkg.license : "see text");
      add(id, id, readFileSync(join(dir, file), "utf8"), `${name} ${pkg.version} (JavaScript)`);
    }
  }

  // --- page ------------------------------------------------------------------------------------
  const ordered = [...groups.values()].sort((a, b) => a.id.localeCompare(b.id) || a.text.localeCompare(b.text));
  const count = new Set(ordered.flatMap((g) => [...g.packages])).size;
  const own_license = normalize(readFileSync("LICENSE", "utf8"));
  const notice = normalize(readFileSync("NOTICE", "utf8"));

  const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>FileEpoch – licenses</title>
<style>
  :root { color-scheme: light dark; font-family: "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif; }
  body { max-width: 900px; margin: 0 auto; padding: 24px 28px 60px; line-height: 1.5; }
  h1 { font-size: 22px; margin: 0 0 4px; }
  h2 { font-size: 17px; margin: 36px 0 8px; padding-top: 12px; border-top: 1px solid #8884; }
  p, li { font-size: 14px; }
  .used { font-size: 13px; opacity: 0.8; }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; font-size: 12px; padding: 12px 14px; border-radius: 6px; background: #8881; }
  nav li { margin: 2px 0; }
</style>
</head>
<body>
<h1>FileEpoch – licenses</h1>
<p>FileEpoch is free software under the Apache License 2.0. It is built on ${count} open-source
components by other authors, listed below with their licenses. Generated from Cargo.lock and the
built user interface.</p>

<h2>FileEpoch</h2>
<pre>${esc(notice)}</pre>
<pre>${esc(own_license)}</pre>

<h2>Third-party components</h2>
<nav><ul>
${ordered.map((g, i) => `<li><a href="#l${i}">${esc(g.name)}</a> – ${g.packages.size} ${g.packages.size === 1 ? "component" : "components"}</li>`).join("\n")}
</ul></nav>
${mpl.size ? `<p>Source code of the components under the Mozilla Public License 2.0, used unmodified: ${[...mpl].sort().map(esc).join(", ")}.</p>` : ""}
${ordered
  .map(
    (g, i) => `
<h2 id="l${i}">${esc(g.name)}</h2>
<p class="used">Used by: ${[...g.packages].sort().map(esc).join(", ")}</p>
<pre>${esc(g.text)}</pre>`,
  )
  .join("\n")}
</body>
</html>
`;

  if (check) {
    const current = existsSync(OUT) ? readFileSync(OUT, "utf8").replace(/\r\n/g, "\n") : "";
    if (current !== html) {
      console.error(`${OUT} is out of date. Run: node tools/third-party-notices.mjs`);
      process.exit(1);
    }
    console.log(`${OUT} is up to date (${count} components).`);
  } else {
    writeFileSync(OUT, html);
    console.log(`Wrote ${OUT} (${count} components, ${ordered.length} license texts).`);
  }
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

function normalize(text) {
  return text.replace(/\r\n/g, "\n").replace(/[ \t]+$/gm, "").replace(/^\n+|\s+$/g, "") + "\n";
}

function esc(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}
