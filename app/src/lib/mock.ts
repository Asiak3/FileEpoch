// Mock backend used when the UI runs in a plain browser (npm run dev), for development,
// screenshots and contributors without a Kopia server. Never loaded inside the Tauri app.
//
// URL parameters: ?screen=connect  ?lang=es|de|en  ?theme=light|dark  ?source=<n>  ?path=a/b
//                 ?mismatch=1  ?readonly=0  ?version=0.99.0

import type { Api, Entry, Listing, Settings, Snapshot, Source } from "./api";

const params = new URLSearchParams(location.search);
const FP_HEX = "3f9a0c5e7b21d4486a0e9f13c2b75d8e41a6f0c97e2b5d3184c6a9e07f1b2d4c";
const FP_COLON = FP_HEX.match(/../g)!.join(":").toUpperCase();
const HOUR = 3600_000;
const DAY = 24 * HOUR;
const NOW = Date.UTC(2026, 10, 25, 20, 0, 0);

const sources: Source[] = [
  { host: "nas", userName: "root", path: "/srv/fotos-familia" },
  { host: "nas", userName: "root", path: "/srv/oficina/documentos" },
  { host: "laptop", userName: "ana", path: "/home/ana" },
];

function hex(n: number, len = 32): string {
  let h = "";
  let x = (n * 2654435761) >>> 0;
  while (h.length < len) {
    x = (x ^ (x << 13)) >>> 0;
    x = (x ^ (x >>> 17)) >>> 0;
    x = (x ^ (x << 5)) >>> 0;
    h += x.toString(16).padStart(8, "0");
  }
  return h.slice(0, len);
}

/** Content version per snapshot: changes every few snapshots so grouping is visible. */
function snapshotsFor(i: number): Snapshot[] {
  const list: { time: number; version: number }[] = [];
  if (i === 0) {
    list.push({ time: NOW - 16 * HOUR, version: 1 });
  } else if (i === 1) {
    // Same spacing as the recorded real server: 3 in the first hour, then every 2 h.
    const base = NOW - 21 * HOUR;
    const offsets = [0, 0.13, 0.18, 4.1, 5, 7, 9, 11, 13, 15, 17, 19];
    const versions = [1, 1, 1, 1, 1, 1, 2, 3, 3, 4, 4, 4];
    offsets.forEach((o, k) => list.push({ time: base + o * HOUR, version: versions[k] }));
  } else {
    // About 1000 snapshots every 2 hours (~83 days).
    for (let k = 0; k < 1000; k++) list.push({ time: NOW - (999 - k) * 2 * HOUR, version: 1 + Math.floor(k / 3) });
  }
  return list.map(({ time, version }, k) => {
    const rootId = `k${i}${hex(version, 31)}`;
    if (!roots.has(rootId)) roots.set(rootId, { source: i, version, time });
    return {
    id: hex(i * 100000 + k),
    startTime: new Date(time).toISOString(),
    endTime: new Date(time + 95_000).toISOString(),
    rootId,
    incomplete: null,
    files: 1200 + version * 3,
    size: 4_800_000_000 + version * 12_345_678,
    retention: k === list.length - 1 ? ["latest-1"] : [],
    pins: [],
    description: "",
    };
  });
}

/** Content version and first appearance of each root, filled by snapshotsFor. */
const roots = new Map<string, { source: number; version: number; time: number }>();
const snapshotCache = sources.map((_, i) => snapshotsFor(i));


function versionOf(root: string): { source: number; version: number; time: number } {
  return roots.get(root) ?? { source: 0, version: 0, time: NOW };
}

function file(name: string, size: number, mtime: number, n: number): Entry {
  return { name, kind: "f", size, mtime, obj: hex(n, 32), mode: n % 5 === 0 ? "0600" : "0644", uid: 1000, gid: 1000 };
}

function dir(name: string, size: number, mtime: number, n: number, files = 12): Entry {
  return { name, kind: "d", size, mtime, obj: "k" + hex(n, 32), files, mode: "0755", uid: n % 3 === 0 ? 0 : 1000, gid: 1000 };
}

/** Directory contents for a path at a given content version. null = does not exist. */
function listing(source: number, version: number, t: number, path: string[]): Entry[] | null {
  const key = path.join("/");
  const seed = (s: string) => [...s].reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7) + version;
  const ago = (h: number) => t - h * HOUR;
  // "Proyectos 2026" appears at content version projFrom; "Proyectos 2026/Diseño" exists only
  // between disFrom and disTo, to exercise "stay in the folder while moving in time".
  const [projFrom, disFrom, disTo] = source === 1 ? [2, 3, 3] : source === 2 ? [5, 8, 9] : [99, 99, 99];
  if (path.length === 0) {
    const root: Entry[] = [
      dir("Documentos", 2_300_000_000, ago(2), 1, 640),
      dir("Fotos", 38_000_000_000, ago(30), 2, 9100),
      dir("Trabajo", 820_000_000, ago(5), 3, 220),
      dir("Facturas_Q4", 12_400_000, ago(52), 4, 38),
      file("Presupuesto_Q4.xlsx", 1_468_006, ago(1.8), 10),
      file("Informe_Final_2025.pdf", 3_355_443, ago(2.7), 11),
      file("Notas_Reunión.txt", 12_288, ago(4), 12),
      file("Foto_Equipo.jpg", 4_299_161, ago(30), 13),
      file("Presentacion_Estrategia.pptx", 9_122_611, ago(80), 14),
      file("Contrato_Firmado.pdf", 2_202_009, ago(125), 15),
      file("Resumen_Ejecutivo.docx", 532_480, ago(150), 16),
      file("backup-vm.qcow2", 64_424_509_440, ago(300), 17),
      file("script:copia?.sh", 2_048, ago(400), 18),
      { name: "actual", kind: "s", size: 9, mtime: ago(500), obj: hex(19) },
    ];
    if (version >= projFrom) root.unshift(dir("Proyectos 2026", 1_900_000_000, ago(1), 5, 410));
    if (source === 2) root.unshift(dir("Carpeta enorme (100 000 archivos)", 9_000_000_000, ago(10), 6, 100_000));
    return root;
  }
  if (key === "Proyectos 2026") {
    if (version < projFrom) return null;
    const items: Entry[] = [dir("Cliente Norte", 300_000_000, t - 3 * DAY, 20, 80)];
    if (version >= disFrom && version <= disTo) items.push(dir("Diseño", 800_000_000, t - DAY, 21, 120));
    items.push(file("Plan.docx", 220_000 + version, t - 2 * HOUR, 22));
    return items;
  }
  if (key === "Carpeta enorme (100 000 archivos)" && source === 2) {
    const out: Entry[] = new Array(100_000);
    for (let k = 0; k < 100_000; k++) {
      out[k] = {
        name: `IMG_${String(k).padStart(6, "0")}.jpg`,
        kind: "f",
        size: 1_000_000 + ((k * 7919) % 5_000_000),
        mtime: t - (k % 5000) * HOUR,
        obj: hex(k + 1_000_000),
      };
    }
    return out;
  }
  if (path.length > 6) return [];
  // Generic folder: a few subfolders and files, stable per path.
  const s = seed(key);
  const out: Entry[] = [];
  const nDirs = s % 4;
  for (let k = 0; k < nDirs; k++) out.push(dir(`Carpeta ${String.fromCharCode(65 + k)}`, 10_000_000 * (k + 1), t - (k + 1) * DAY, s + k));
  const names = ["informe", "factura", "foto", "notas", "plano", "contrato", "acta", "datos"];
  const exts = ["pdf", "jpg", "docx", "xlsx", "txt", "png", "zip", "mp4"];
  for (let k = 0; k < 6 + (s % 9); k++) {
    out.push(file(`${names[(s + k) % names.length]}_${2020 + ((s + k) % 7)}_${k + 1}.${exts[(s + k * 3) % exts.length]}`, 4096 + ((s * (k + 3)) % 20_000_000), t - ((s + k) % 900) * HOUR, s * 10 + k));
  }
  return out;
}

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function mockApi(): Api {
  const screen = params.get("screen");
  const settings: Settings = {
    server: screen === "connect" ? "" : "https://backup.example:51515",
    user: screen === "connect" ? "" : "kopia",
    fingerprint: screen === "connect" ? "" : FP_HEX,
    language: (params.get("lang") as Settings["language"]) ?? "auto",
    theme: (params.get("theme") as Settings["theme"]) ?? "system",
    groupIdentical: params.get("all") !== "1",
    lastSource: "",
    viewMode: (params.get("view") as Settings["viewMode"]) ?? "details",
    columns: params.get("cols") === "all" ? [
      { id: "mtime", visible: true, width: 170 },
      { id: "type", visible: true, width: 150 },
      { id: "size", visible: true, width: 90 },
      { id: "items", visible: true, width: 80 },
      { id: "mode", visible: true, width: 100 },
      { id: "owner", visible: true, width: 120 },
    ] : [],
    restoreNaming: "suffix",
    suffixDate: "snapshot",
    conflict: (params.get("conflict") as Settings["conflict"]) ?? "rename",
    verify: true,
    lastRestoreDir: "",
  };
  const sourceIndex = Number(params.get("source") ?? "1");
  settings.lastSource = `${sources[sourceIndex].userName}@${sources[sourceIndex].host}:${sources[sourceIndex].path}`;
  let savedPassword = screen === "connect" ? null : "demo";

  return {
    async loadSettings() {
      return { ...settings };
    },
    async saveSettings(s) {
      Object.assign(settings, s);
    },
    async probeCertificate(server) {
      await delay(300);
      if (!server.startsWith("https://")) throw { code: "invalid_server", message: server };
      return { hex: FP_HEX, colon: FP_COLON };
    },
    async connect(req) {
      await delay(400);
      if (params.get("mismatch") === "1") {
        throw { code: "certificate_mismatch", message: "mismatch", presented: hex(99, 64) };
      }
      if (req.fingerprint.toLowerCase() !== FP_HEX) {
        throw { code: "certificate_mismatch", message: "mismatch", presented: FP_HEX };
      }
      const password = req.password ?? savedPassword;
      if (password === null) throw { code: "password_required", message: "" };
      if (password !== "demo") {
        throw req.password === null ? { code: "saved_password_rejected", message: "" } : { code: "access_denied", message: "" };
      }
      if (req.remember) savedPassword = password;
      const version = params.get("version") ?? "0.23.1";
      return {
        version,
        testedVersions: ["0.23.1"],
        untestedVersion: version !== "0.23.1",
        readonly: params.get("readonly") !== "0",
        description: "Copias de casa",
      };
    },
    async disconnect() {},
    async forgetPassword() {
      const had = savedPassword !== null;
      savedPassword = null;
      return had;
    },
    async listSources() {
      await delay(150);
      return sources;
    },
    async listSnapshots(source) {
      await delay(200);
      return snapshotCache[sources.findIndex((s) => s.path === source.path)] ?? [];
    },
    async openPath(root, path): Promise<Listing> {
      const { source, version, time } = versionOf(root);
      await delay(path[0]?.startsWith("Carpeta enorme") ? 500 : 120);
      // Same rule as KopiaClient::resolve_path: follow folders while they exist.
      let depth = 0;
      let entries = listing(source, version, time, [])!;
      for (const name of path) {
        if (!entries.some((e) => e.name === name && e.kind === "d")) break;
        const next = listing(source, version, time, path.slice(0, depth + 1));
        if (next === null) break;
        depth++;
        entries = next;
      }
      return { depth, entries };
    },
    async setWindowTheme() {},
  };
}
