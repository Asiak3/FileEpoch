<script lang="ts">
  import type { FileCategory } from "../filetypes";

  let { category, size = 20 }: { category: FileCategory; size?: number } = $props();

  // Accent colour per document type (label band / glyph).
  const colors: Partial<Record<FileCategory, string>> = {
    pdf: "#d83b2d",
    doc: "#2b6fd6",
    sheet: "#1e8a4c",
    slides: "#d9622b",
    image: "#0d9488",
    video: "#7c4dd8",
    audio: "#c2418d",
    archive: "#9a6b2f",
    text: "#6b7280",
    code: "#475569",
    disk: "#5b6472",
    app: "#2563eb",
    link: "#0891b2",
    file: "#8a93a0",
  };
  const labels: Partial<Record<FileCategory, string>> = {
    pdf: "PDF",
    doc: "W",
    sheet: "X",
    slides: "P",
  };
  let color = $derived(colors[category] ?? "#8a93a0");
</script>

<svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
  {#if category === "folder"}
    <path d="M2.5 6.2c0-1 .8-1.7 1.7-1.7h4.6c.5 0 .9.2 1.2.5l1.4 1.5h8.4c1 0 1.7.8 1.7 1.7v9.6c0 1-.8 1.7-1.7 1.7H4.2c-1 0-1.7-.8-1.7-1.7V6.2Z" fill="#e8a317" />
    <path d="M2.5 9.2c0-1 .8-1.7 1.7-1.7h15.6c1 0 1.7.8 1.7 1.7v8.6c0 1-.8 1.7-1.7 1.7H4.2c-1 0-1.7-.8-1.7-1.7V9.2Z" fill="#fcc94a" />
    <path d="M2.5 9.2c0-1 .8-1.7 1.7-1.7h15.6c1 0 1.7.8 1.7 1.7v.6H2.5v-.6Z" fill="#ffdc7a" />
  {:else}
    <path d="M6 2.5h8.2L19.5 7.8v12c0 1-.8 1.7-1.7 1.7H6c-1 0-1.7-.8-1.7-1.7V4.2c0-1 .8-1.7 1.7-1.7Z" fill="var(--file-paper, #fff)" stroke="var(--file-edge, #c5cbd4)" stroke-width="1" />
    <path d="M14.2 2.5v3.6c0 1 .8 1.7 1.7 1.7h3.6" fill="none" stroke="var(--file-edge, #c5cbd4)" stroke-width="1" />
    {#if labels[category]}
      <rect x="2.5" y="11.5" width={category === "pdf" ? 13 : 9.5} height="7.5" rx="1.6" fill={color} />
      <text x={category === "pdf" ? 9 : 7.25} y="17.1" text-anchor="middle" font-size={category === "pdf" ? 5.4 : 6} font-weight="700" fill="#fff" font-family="Segoe UI, sans-serif">{labels[category]}</text>
    {:else if category === "image"}
      <circle cx="9.2" cy="11.2" r="1.5" fill={color} />
      <path d="M6.5 18.5l3.6-4 2.2 2.3 2.4-3.1 3 4.8Z" fill={color} />
    {:else if category === "video"}
      <path d="M9.5 10.5v6.5l5.5-3.25Z" fill={color} />
    {:else if category === "audio"}
      <path d="M10.5 16.5V10l5-1.2v6.2" fill="none" stroke={color} stroke-width="1.4" />
      <circle cx="9.3" cy="16.6" r="1.4" fill={color} />
      <circle cx="14.3" cy="15.1" r="1.4" fill={color} />
    {:else if category === "archive"}
      <path d="M11 3v2h1.5v2H11v2h1.5v2H11v2" stroke={color} stroke-width="1.3" fill="none" />
      <rect x="9.8" y="13" width="3.6" height="4" rx="0.8" fill={color} />
    {:else if category === "code"}
      <path d="M10 11.5 7.5 14l2.5 2.5M14 11.5l2.5 2.5-2.5 2.5" fill="none" stroke={color} stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
    {:else if category === "disk"}
      <circle cx="12" cy="14.2" r="4" fill="none" stroke={color} stroke-width="1.3" />
      <circle cx="12" cy="14.2" r="1" fill={color} />
    {:else if category === "link"}
      <path d="M9 17c0-3 2-4.8 5.5-4.8M12.8 10l2 2.2-2 2.2" fill="none" stroke={color} stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
    {:else if category === "app"}
      <rect x="8" y="10.5" width="8" height="7" rx="1.2" fill="none" stroke={color} stroke-width="1.3" />
      <path d="M8 12.6h8" stroke={color} stroke-width="1.3" />
    {:else}
      <path d="M8 11.5h8M8 14h8M8 16.5h5" stroke={color} stroke-width="1.2" stroke-linecap="round" />
    {/if}
  {/if}
</svg>
