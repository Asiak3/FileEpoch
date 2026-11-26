// File type classification for icons and the "Type" column.

import type { Entry } from "./api";

export type FileCategory =
  | "folder"
  | "link"
  | "pdf"
  | "doc"
  | "sheet"
  | "slides"
  | "image"
  | "video"
  | "audio"
  | "archive"
  | "text"
  | "code"
  | "disk"
  | "app"
  | "file";

const byExt: Record<string, FileCategory> = {};
const groups: [FileCategory, string][] = [
  ["pdf", "pdf"],
  ["doc", "doc docx odt rtf pages"],
  ["sheet", "xls xlsx xlsm ods csv numbers"],
  ["slides", "ppt pptx odp key"],
  ["image", "jpg jpeg png gif bmp tif tiff webp heic heif raw cr2 cr3 nef arw dng svg ico psd avif"],
  ["video", "mp4 mov avi mkv wmv m4v webm mpg mpeg 3gp mts"],
  ["audio", "mp3 wav flac aac ogg m4a wma opus"],
  ["archive", "zip 7z rar tar gz tgz bz2 xz zst"],
  ["text", "txt md log ini conf cfg nfo"],
  ["code", "js ts jsx tsx py rs go java c cpp h hpp cs php rb sh ps1 bat json xml yaml yml toml html css sql"],
  ["disk", "iso img vhd vhdx vmdk vdi qcow2"],
  ["app", "exe msi dll app dmg deb rpm apk"],
];
for (const [cat, exts] of groups) for (const e of exts.split(" ")) byExt[e] = cat;

export function extension(name: string): string {
  const i = name.lastIndexOf(".");
  return i > 0 && i < name.length - 1 ? name.slice(i + 1).toLowerCase() : "";
}

export function category(e: Pick<Entry, "name" | "kind">): FileCategory {
  if (e.kind === "d") return "folder";
  if (e.kind === "s") return "link";
  return byExt[extension(e.name)] ?? "file";
}
