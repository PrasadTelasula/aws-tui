/**
 * Central font registry.
 *
 * Font *files* are imported exactly once in `src/app.css`. This module exposes
 * the logical font stack identifiers and utilities so any code that needs to
 * reference a font does so through a single typed surface.
 *
 * To add a new font family:
 *   1. Install the `@fontsource-variable/<name>` package (or place files in
 *      `static/fonts/` and add `@font-face` rules to `app.css`).
 *   2. Import its CSS in `src/app.css` — NOT here, NOT in components.
 *   3. Add an entry to `fontStacks` below.
 *   4. Add a CSS variable in `app.css` under `:root`.
 *   5. Extend `tailwind.config.js` `fontFamily` if it needs a utility class.
 *
 * No component should ever hard-code a `font-family` CSS value. Use the
 * `font-sans` / `font-mono` Tailwind classes, or CSS variables from this map.
 */

export const fontStacks = {
  sans: 'var(--font-sans)',
  mono: 'var(--font-mono)'
} as const;

export type FontStack = keyof typeof fontStacks;

export function fontFamily(stack: FontStack): string {
  return fontStacks[stack];
}
