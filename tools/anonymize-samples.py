#!/usr/bin/env python3
"""Turn real API responses saved by tools/verify-api.ps1 into anonymized test fixtures.

Usage:  python -I tools/anonymize-samples.py [local/api-samples] [crates/kopia-client/tests/fixtures]

Replaces every name, path, host, user, description, config path and ID with synthetic values,
consistently across files (the same original value always maps to the same replacement, so
grouping by rootID still works). Timestamps, sizes, counts and retention labels are kept.
At the end it checks that no original value appears in the output, and fails otherwise.
"""
import hashlib
import json
import os
import re
import secrets
import sys

SRC = sys.argv[1] if len(sys.argv) > 1 else os.path.join("local", "api-samples")
DST = sys.argv[2] if len(sys.argv) > 2 else os.path.join("crates", "kopia-client", "tests", "fixtures")
FILES = ["repo-status.json", "sources.json", "dir-root.json"]

SALT = secrets.token_bytes(16)
originals = set()          # every replaced value, for the final leak check
maps = {}                  # kind -> {original: replacement}
OID_RE = re.compile(r"^(I*Z?[g-z]?)([0-9a-f]+)$")


def mapped(kind, value, make):
    table = maps.setdefault(kind, {})
    if value not in table:
        table[value] = make(len(table) + 1)
    if value:
        originals.add(value)
    return table[value]


def fake_hex(value, length):
    return hashlib.sha256(SALT + value.encode()).hexdigest()[:length]


def oid(value):
    m = OID_RE.match(value)
    if not m:
        raise SystemExit(f"unexpected object ID format: {value!r}")
    return mapped("oid", value, lambda n: m.group(1) + fake_hex(value, len(m.group(2))))


def snapshot_id(value):
    return mapped("sid", value, lambda n: fake_hex(value, len(value)))


def source_path(value):
    return mapped("path", value, lambda n: f"/data/source-{n}")


def entry_name(entry):
    name, kind = entry.get("name", ""), entry.get("type")
    if kind == "d":
        return mapped("name", name, lambda n: f"dir-{n:02d}")
    if kind == "s":
        return mapped("name", name, lambda n: f"link-{n:02d}")
    ext = os.path.splitext(name)[1].lower()
    ext = ext if re.fullmatch(r"\.[a-z0-9]{1,5}", ext) else ""
    return mapped("name", name, lambda n: f"file-{n:02d}{ext}")


def summary(s):
    if isinstance(s, dict):
        s.pop("errors", None)  # failed entries carry real paths
    return s


def dir_entry(e, name=None):
    e["name"] = name if name is not None else entry_name(e)
    if "obj" in e:
        e["obj"] = oid(e["obj"])
    summary(e.get("summ"))
    return e


def source(s):
    s["host"] = mapped("host", s["host"], lambda n: f"host{n}")
    s["userName"] = mapped("user", s["userName"], lambda n: f"user{n}")
    s["path"] = source_path(s["path"])
    return s


def anonymize(name, data):
    if name == "repo-status.json":
        data["hostname"] = mapped("host", data.get("hostname", ""), lambda n: f"host{n}")
        data["username"] = mapped("user", data.get("username", ""), lambda n: f"user{n}")
        if data.get("configFile"):
            originals.add(data["configFile"])
            data["configFile"] = "/config/repository.config"
        if data.get("description"):
            originals.add(data["description"])
            data["description"] = "Test repository"
    elif name == "sources.json":
        data["localHost"] = mapped("host", data.get("localHost", ""), lambda n: f"host{n}")
        data["localUsername"] = mapped("user", data.get("localUsername", ""), lambda n: f"user{n}")
        for st in data.get("sources", []):
            source(st["source"])
            ls = st.get("lastSnapshot")
            if ls:
                ls["id"] = snapshot_id(ls["id"])
                source(ls["source"])
                ls.pop("tags", None)
                if ls.get("description"):
                    originals.add(ls["description"])
                    ls["description"] = ""
                if ls.get("rootEntry"):
                    dir_entry(ls["rootEntry"], name=st["source"]["path"].rsplit("/", 1)[-1])
    elif name.startswith("snapshots"):
        for s in data.get("snapshots", []):
            s["id"] = snapshot_id(s["id"])
            s["rootID"] = oid(s["rootID"])
            if s.get("description"):
                originals.add(s["description"])
                s["description"] = ""
            summary(s.get("summary"))
    elif name == "dir-root.json":
        for e in data.get("entries") or []:
            dir_entry(e)
        summary(data.get("summary"))
    return data


def string_values(node):
    """All string values (not keys) in a JSON document."""
    if isinstance(node, dict):
        for v in node.values():
            yield from string_values(v)
    elif isinstance(node, list):
        for v in node:
            yield from string_values(v)
    elif isinstance(node, str):
        yield node


def main():
    names = FILES + sorted(f for f in os.listdir(SRC) if f.startswith("snapshots"))
    # sources first so source paths get stable numbers
    names.sort(key=lambda f: (f != "sources.json", f))
    os.makedirs(DST, exist_ok=True)
    outputs, values = {}, {}
    for name in names:
        with open(os.path.join(SRC, name), encoding="utf-8") as fh:
            data = json.load(fh)
        data = anonymize(name, data)
        outputs[name] = json.dumps(data, indent=2, ensure_ascii=False) + "\n"
        values[name] = list(string_values(data))

    # Leak check: no original value (or path component of one) may survive.
    needles = set()
    for v in originals:
        if not isinstance(v, str) or not v:
            continue
        needles.add(v)
        needles.update(p for p in re.split(r"[/\\:@]", v) if len(p) >= 3)
    # Generic fragments of Kopia's default config path that also occur in fixed values
    # such as "kopia:directory" or the synthetic "/config/repository.config".
    needles -= {"kopia", ".config", "config", "repository.config"}
    leaks = [(n, f) for n in sorted(needles) for f, vals in values.items() if any(n in v for v in vals)]
    if leaks:
        for n, f in leaks:
            print(f"LEAK in {f}: original value fragment still present", file=sys.stderr)
        raise SystemExit("anonymization failed; nothing written")

    for name, text in outputs.items():
        with open(os.path.join(DST, name), "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)
    print(f"wrote {len(outputs)} fixtures to {DST}; checked {len(needles)} original values for leaks")


if __name__ == "__main__":
    main()
