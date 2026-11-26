<script lang="ts" module>
  // Simple 20×20 line icons in the spirit of Fluent UI System Icons (drawn for Rewind).
  const paths: Record<string, string> = {
    "chevron-left": "M12.5 4.5 7 10l5.5 5.5",
    "chevron-right": "M7.5 4.5 13 10l-5.5 5.5",
    "chevron-down": "M5 7.5 10 12.5l5-5",
    "chevron-up": "M5 12.5 10 7.5l5 5",
    "arrow-left": "M16 10H4.5M9 5 4 10l5 5",
    "arrow-right": "M4 10h11.5M11 5l5 5-5 5",
    "arrow-up": "M10 16V4.5M5 9l5-5 5 5",
    search: "M8.5 14a5.5 5.5 0 1 0 0-11 5.5 5.5 0 0 0 0 11ZM12.5 12.5 17 17",
    x: "M5 5l10 10M15 5 5 15",
    check: "M4.5 10.5 8 14l7.5-8",
    settings:
      "M10 12.75a2.75 2.75 0 1 0 0-5.5 2.75 2.75 0 0 0 0 5.5ZM8.3 2.6l-.4 1.9a6 6 0 0 0-1.5.9L4.6 4.8 2.9 7.7l1.4 1.3a6 6 0 0 0 0 2l-1.4 1.3 1.7 2.9 1.8-.6a6 6 0 0 0 1.5.9l.4 1.9h3.4l.4-1.9a6 6 0 0 0 1.5-.9l1.8.6 1.7-2.9-1.4-1.3a6 6 0 0 0 0-2l1.4-1.3-1.7-2.9-1.8.6a6 6 0 0 0-1.5-.9l-.4-1.9Z",
    warning: "M10 3 2.5 16.5h15L10 3ZM10 8v4M10 14.2v.3",
    info: "M10 17.5a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15ZM10 9v5M10 6.2v.3",
    shield: "M10 2.5 4 4.75v4.6c0 3.7 2.6 6.6 6 8.15 3.4-1.55 6-4.45 6-8.15v-4.6L10 2.5ZM7.25 10l2 2 3.5-4",
    "shield-x": "M10 2.5 4 4.75v4.6c0 3.7 2.6 6.6 6 8.15 3.4-1.55 6-4.45 6-8.15v-4.6L10 2.5ZM8 8l4 4M12 8l-4 4",
    server: "M4 3.5h12v5H4zM4 11.5h12v5H4zM6.5 6h.01M6.5 14h.01",
    key: "M12.5 11a4 4 0 1 0-3.9-3.1L3.5 13v3.5H7v-2h2v-2h1.6l.1-.1a4 4 0 0 0 1.8.6ZM13.5 6.5h.01",
    plug: "M7 2.5v4M13 2.5v4M5.5 6.5h9V9a4.5 4.5 0 0 1-9 0V6.5ZM10 13.5v4",
    clock: "M10 17.5a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15ZM10 6v4.5l3 2",
    layers: "M10 3 2.5 7 10 11l7.5-4L10 3ZM2.5 10.5 10 14.5l7.5-4M2.5 14 10 18l7.5-4",
    home: "M3.5 9 10 3.5 16.5 9v7.5h-4.25v-5h-4.5v5H3.5V9Z",
    globe:
      "M10 17.5a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15ZM2.5 10h15M10 2.5c2 2.2 3 4.7 3 7.5s-1 5.3-3 7.5c-2-2.2-3-4.7-3-7.5s1-5.3 3-7.5Z",
    "view-details": "M3 5h2M8 5h9M3 10h2M8 10h9M3 15h2M8 15h9",
    "view-list": "M3.5 5h13M3.5 10h13M3.5 15h13",
    "view-icons": "M3.5 3.5h5v5h-5zM11.5 3.5h5v5h-5zM3.5 11.5h5v5h-5zM11.5 11.5h5v5h-5z",
    "view-tiles": "M3 4h5v4.5H3zM10.5 5h6.5M10.5 7.5h4.5M3 11.5h5V16H3zM10.5 12.5h6.5M10.5 15h4.5",
    restore: "M3.5 10a6.5 6.5 0 1 0 2-4.7M3.5 3.5v3.5H7M10 6.5V10l2.5 1.5",
    copy: "M7 7h9.5v9.5H7zM13 7V3.5H3.5V13H7",
    "folder-open": "M2.5 6V4.5h5l1.5 2h7.5V9M2.5 6v10h12.5l2.5-7H5L2.5 16",
    "select-all": "M3.5 3.5h13v13h-13zM7 10l2 2 4-4.5",
    download: "M10 3v10M5.5 8.5 10 13l4.5-4.5M3.5 16.5h13",
    folder: "M2.5 5.5v10h15V7.5H9.5L8 5.5H2.5Z",
    trash: "M4 6h12M8 6V4h4v2M5.5 6l.8 10.5h7.4L14.5 6",
    stop: "M5 5h10v10H5z",
    monitor: "M2.5 4h15v9.5h-15zM7 16.5h6M10 13.5v3",
    external: "M11 3.5h5.5V9M16.5 3.5 9 11M14.5 12v4.5h-11v-11H8",
    palette:
      "M10 17.5a7.5 7.5 0 1 1 7.5-7.5c0 2-1.6 3-3.2 3H12.6a1.6 1.6 0 0 0-1.2 2.7c.6.7.1 1.8-.8 1.8H10ZM6.5 9.5h.01M8.5 6.5h.01M12 6.5h.01M14 9.5h.01",
  };
</script>

<script lang="ts">
  let { name, size = 18, stroke = 1.5 }: { name: string; size?: number; stroke?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 20 20"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  <path d={paths[name] ?? ""} />
</svg>
