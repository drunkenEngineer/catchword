// How the interface looks (APP-3): the theme and text size the user chose,
// set on the root element, where styles.css reads them.
import type { TextSize } from "./contract/TextSize";
import type { Theme } from "./contract/Theme";

export function applyAppearance(theme: Theme, textSize: TextSize): void {
  const root = document.documentElement;
  if (theme === "system") delete root.dataset.theme;
  else root.dataset.theme = theme;
  if (textSize === "normal") delete root.dataset.textSize;
  else root.dataset.textSize = textSize;
}
