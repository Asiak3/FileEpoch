// Closes a popover on outside click or Escape. Returns a cleanup function for $effect.
export function dismissable(root: HTMLElement, close: () => void, ignore?: HTMLElement): () => void {
  const onPointer = (e: PointerEvent) => {
    const target = e.target as Node;
    if (!root.contains(target) && !ignore?.contains(target)) close();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      close();
      (root.querySelector("[aria-haspopup]") as HTMLElement | null)?.focus();
    }
  };
  document.addEventListener("pointerdown", onPointer, true);
  root.addEventListener("keydown", onKey);
  return () => {
    document.removeEventListener("pointerdown", onPointer, true);
    root.removeEventListener("keydown", onKey);
  };
}

/** Arrow-key navigation between the focusable items of a menu/list. */
export function arrowNav(e: KeyboardEvent, container: HTMLElement, selector: string) {
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "Home" && e.key !== "End") return;
  const items = [...container.querySelectorAll<HTMLElement>(selector)];
  if (items.length === 0) return;
  e.preventDefault();
  const i = items.indexOf(document.activeElement as HTMLElement);
  const next =
    e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : e.key === "ArrowDown" ? Math.min(items.length - 1, i + 1) : Math.max(0, i - 1);
  items[next].focus();
}
