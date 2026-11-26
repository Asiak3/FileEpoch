// Interface between the UI and the Rust backend (src-tauri/src/commands.rs).
// Outside Tauri (plain browser during development) a mock backend is loaded instead.

export type Language = "auto" | "es" | "de" | "en";
export type Theme = "system" | "light" | "dark";

export type ViewMode = "details" | "list" | "icons" | "tiles";
export type ColumnId = "mtime" | "type" | "size" | "mode" | "owner" | "items";

export interface ColumnSetting {
  id: ColumnId;
  visible: boolean;
  width: number;
}

export interface Settings {
  server: string;
  user: string;
  fingerprint: string;
  language: Language;
  theme: Theme;
  groupIdentical: boolean;
  lastSource: string;
  viewMode: ViewMode;
  columns: ColumnSetting[];
  restoreNaming: "suffix" | "original";
  suffixDate: "snapshot" | "modified";
  conflict: "rename" | "skip" | "overwrite";
  verify: boolean;
  lastRestoreDir: string;
}

export interface Source {
  host: string;
  userName: string;
  path: string;
}

export interface Snapshot {
  id: string;
  /** RFC 3339, UTC. */
  startTime: string;
  endTime: string | null;
  rootId: string;
  incomplete: string | null;
  files: number;
  size: number;
  retention: string[];
  pins: string[];
  description: string;
}

export type EntryKind = "f" | "d" | "s" | "?";

export interface Entry {
  name: string;
  kind: EntryKind;
  size: number;
  /** Milliseconds since the epoch (UTC). */
  mtime: number | null;
  obj: string;
  files?: number;
  /** Octal permissions such as "0644". */
  mode?: string;
  uid?: number;
  gid?: number;
}

export interface Listing {
  /** How many components of the requested path exist in this snapshot. */
  depth: number;
  entries: Entry[];
}

export interface CertInfo {
  hex: string;
  colon: string;
}

export interface ConnectRequest {
  server: string;
  user: string;
  fingerprint: string;
  /** null: use the password saved in the system credential store. */
  password: string | null;
  remember: boolean;
}

export interface ConnectResult {
  version: string | null;
  testedVersions: string[];
  untestedVersion: boolean;
  readonly: boolean;
  description: string;
}

export type ErrorCode =
  | "invalid_server"
  | "invalid_fingerprint"
  | "certificate_mismatch"
  | "connection"
  | "access_denied"
  | "not_ui_user"
  | "session"
  | "server_not_connected"
  | "not_found"
  | "incompatible"
  | "credential_store"
  | "password_required"
  | "saved_password_rejected"
  | "not_connected_app"
  | "other";

export interface ApiError {
  code: ErrorCode;
  message: string;
  presented?: string;
}

export interface RestoreRequest {
  source: Source;
  snapshotRoot: string;
  snapshotTime: string;
  /** Folder shown (names below the snapshot root). */
  path: string[];
  /** Selected names in that folder; empty = the folder itself. */
  names: string[];
  dest: string;
  naming: Settings["restoreNaming"];
  suffixDate: Settings["suffixDate"];
  label: string;
  conflict: Settings["conflict"];
  overwriteConfirmed: boolean;
  verify: boolean;
}

export interface RestoreSummary {
  topLevel: string[];
  filesRestored: number;
  bytesRestored: number;
  dirsCreated: number;
  renamed: [string, string][];
  renamedCount: number;
  skippedLinks: string[];
  skippedLinksCount: number;
  skippedExisting: string[];
  skippedExistingCount: number;
  errors: [string, string][];
  errorsCount: number;
}

export interface RestoreJob {
  id: number;
  state: "queued" | "running" | "done" | "cancelled" | "failed";
  phase: "scanning" | "downloading" | "finishing";
  sourcePath: string;
  snapshotTime: string;
  items: string[];
  itemCount: number;
  dest: string;
  dirsScanned: number;
  filesTotal: number;
  filesDone: number;
  bytesTotal: number;
  bytesDone: number;
  current: string;
  summary: RestoreSummary | null;
  error: string | null;
}

export interface Api {
  loadSettings(): Promise<Settings>;
  saveSettings(settings: Settings): Promise<void>;
  probeCertificate(server: string): Promise<CertInfo>;
  connect(request: ConnectRequest): Promise<ConnectResult>;
  disconnect(): Promise<void>;
  forgetPassword(server: string, user: string): Promise<boolean>;
  listSources(): Promise<Source[]>;
  listSnapshots(source: Source): Promise<Snapshot[]>;
  openPath(root: string, path: string[]): Promise<Listing>;
  setWindowTheme(theme: "light" | "dark" | null): Promise<void>;
  chooseDestination(start: string | null, title: string): Promise<string | null>;
  restoreConflicts(request: RestoreRequest): Promise<string[]>;
  startRestore(request: RestoreRequest): Promise<number>;
  listRestores(): Promise<RestoreJob[]>;
  cancelRestore(id: number): Promise<void>;
  clearRestores(): Promise<void>;
  openRestoreFolder(id: number): Promise<void>;
  /** Subscribes to job updates; returns an unsubscribe function. */
  onRestoreProgress(callback: (job: RestoreJob) => void): Promise<() => void>;
}

export function isApiError(e: unknown): e is ApiError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

export function toApiError(e: unknown): ApiError {
  if (isApiError(e)) return e;
  return { code: "other", message: e instanceof Error ? e.message : String(e) };
}

export function sourceKey(s: Source): string {
  return `${s.userName}@${s.host}:${s.path}`;
}

async function tauriApi(): Promise<Api> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const { listen } = await import("@tauri-apps/api/event");
  return {
    loadSettings: () => invoke("load_settings"),
    saveSettings: (settings) => invoke("save_settings", { settings }),
    probeCertificate: (server) => invoke("probe_certificate", { server }),
    connect: (request) => invoke("connect", { request }),
    disconnect: () => invoke("disconnect"),
    forgetPassword: (server, user) => invoke("forget_password", { server, user }),
    listSources: () => invoke("list_sources"),
    listSnapshots: (source) => invoke("list_snapshots", { source }),
    openPath: (root, path) => invoke("open_path", { root, path }),
    setWindowTheme: (theme) => getCurrentWindow().setTheme(theme),
    chooseDestination: (start, title) => invoke("choose_destination", { start, title }),
    restoreConflicts: (request) => invoke("restore_conflicts", { request }),
    startRestore: (request) => invoke("start_restore", { request }),
    listRestores: () => invoke("list_restores"),
    cancelRestore: (id) => invoke("cancel_restore", { id }),
    clearRestores: () => invoke("clear_restores"),
    openRestoreFolder: (id) => invoke("open_restore_folder", { id }),
    onRestoreProgress: (callback) => listen<RestoreJob>("restore-progress", (e) => callback(e.payload)),
  };
}

export async function loadApi(): Promise<Api> {
  if ("__TAURI_INTERNALS__" in window) return tauriApi();
  const { mockApi } = await import("./mock");
  return mockApi();
}
