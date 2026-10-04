// F6 and Shift+F6 move the focus between the window's panes (spec section
// 8, keyboard): the navigation, then each part of the destination shown,
// such as Search's box, results and preview. A pane is an element marked
// `data-pane`, in page order.

const CONTROLS = "button, input, select, textarea, [tabindex]";

/** Move the focus to the next pane (`step` 1) or the previous one (-1). */
export function movePane(step: 1 | -1, root: ParentNode = document) {
  const panes = [...root.querySelectorAll<HTMLElement>("[data-pane]")];
  if (panes.length === 0) return;
  const active = document.activeElement;
  const current = panes.findIndex((pane) => pane === active || pane.contains(active));
  const next =
    current < 0
      ? panes[step > 0 ? 0 : panes.length - 1]
      : panes[(current + step + panes.length) % panes.length];
  target(next)?.focus();
}

/** The pane itself if it takes the focus, else its current or first control. */
function target(pane: HTMLElement): HTMLElement | null {
  if (pane.matches(CONTROLS)) return pane;
  return pane.querySelector<HTMLElement>('[aria-current="page"]') ?? pane.querySelector<HTMLElement>(CONTROLS);
}
