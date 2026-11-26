<script lang="ts" module>
  export interface MenuItem {
    label?: string;
    icon?: string;
    action?: () => void;
    disabled?: boolean;
    /** Shows a check mark (toggle items). */
    checked?: boolean;
    separator?: boolean;
    /** Emphasized (primary) item. */
    primary?: boolean;
  }
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import { arrowNav } from "../popover";
  import Icon from "./Icon.svelte";

  let { x, y, items, onclose }: { x: number; y: number; items: MenuItem[]; onclose: () => void } = $props();

  let menu: HTMLDivElement | undefined = $state();
  let left = $state(0);
  let top = $state(0);

  onMount(() => {
    left = x;
    top = y;
    void tick().then(() => {
      if (!menu) return;
      // Keep the menu inside the window.
      const r = menu.getBoundingClientRect();
      if (left + r.width > window.innerWidth - 8) left = Math.max(8, window.innerWidth - r.width - 8);
      if (top + r.height > window.innerHeight - 8) top = Math.max(8, y - r.height);
      (menu.querySelector("[role^=menuitem]:not([disabled])") as HTMLElement | null)?.focus();
    });
    const onPointer = (e: PointerEvent) => {
      if (menu && !menu.contains(e.target as Node)) onclose();
    };
    const onBlur = () => onclose();
    document.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("blur", onBlur);
    window.addEventListener("resize", onBlur);
    return () => {
      document.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("resize", onBlur);
    };
  });

  function run(item: MenuItem) {
    if (item.disabled) return;
    onclose();
    item.action?.();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      onclose();
      return;
    }
    if (menu) arrowNav(e, menu, "[role^=menuitem]:not([disabled])");
  }
</script>

<div
  class="menu"
  role="menu"
  tabindex="-1"
  bind:this={menu}
  style:left="{left}px"
  style:top="{top}px"
  {onkeydown}
  oncontextmenu={(e) => e.preventDefault()}
>
  {#each items as item, i (i)}
    {#if item.separator}
      <div class="sep" role="separator"></div>
    {:else}
      <button
        class="item"
        class:primary={item.primary}
        role={item.checked === undefined ? "menuitem" : "menuitemcheckbox"}
        aria-checked={item.checked === undefined ? undefined : item.checked}
        disabled={item.disabled}
        onclick={() => run(item)}
      >
        <span class="icon">
          {#if item.checked !== undefined}
            {#if item.checked}<Icon name="check" size={15} />{/if}
          {:else if item.icon}
            <Icon name={item.icon} size={16} />
          {/if}
        </span>
        <span>{item.label}</span>
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 50;
    min-width: 220px;
    max-width: 340px;
    padding: 5px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    animation: pop 0.1s var(--ease);
    outline: none;
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.98);
    }
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 32px;
    padding: 0 12px 0 8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
    font-size: 13.5px;
    white-space: nowrap;
  }

  .item:hover:not(:disabled),
  .item:focus-visible {
    background: var(--hover);
    outline: none;
  }

  .item:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .item.primary {
    font-weight: 600;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 18px;
    color: var(--text-2);
    flex: none;
  }

  .sep {
    height: 1px;
    margin: 5px 6px;
    background: var(--border);
  }
</style>
