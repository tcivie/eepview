# Brand

Status: shipped.

The eepview logo and app icons. The mark is a hand-drawn garlic bulb on a pine-green rounded square. The garlic refers to I2P's garlic routing. The wordmark is "eepview" in a monoline geometric lowercase.

All artwork in `assets/brand/` is original work for eepview, released under the repository's MIT license. It reuses no I2P artwork, and the wordmark does not use the I2P name.

## Files

All files are in `assets/brand/`.

| File | Use |
| --- | --- |
| `eepview-mark.svg` | Master mark, on the 1024 px macOS grid (824 px body, radius 185) |
| `eepview-mark-small.svg` | Small variant, for 16–32 px |
| `eepview-mark-mono.svg` | Single-colour variant (`currentColor`), for toolbars and the tray |
| `eepview-logo-horizontal-light.svg`, `eepview-logo-horizontal-dark.svg` | Mark plus wordmark, for light and dark backgrounds |
| `app-icon-1024.png` | The master mark rendered at 1024 px with a soft drop shadow, the source for the icon set |
| `preview.html`, `preview-light.png`, `preview-dark.png` | Preview sheet: master, small 64/48/32/24/16, mono, horizontal logo |

Open a preview page with `#light` or `#dark` to force a theme.

## Palette

| Token | Hex | Use |
| --- | --- | --- |
| pine-700 | `#2A6A56` | icon body top |
| pine-900 | `#0F3529` | icon body bottom |
| pine-950 | `#0D2A23` | dark surfaces of the artwork |
| text-on-light | `#12302A` | wordmark on light |
| paper | `#F3EDE0` | light surface of the artwork |
| text-on-dark | `#EEF2EA` | wordmark on dark |
| accent (light theme) | `#23896A` | i-dot, links, focus |
| accent (dark theme) | `#6CCFA5` | i-dot, links, focus |
| garlic ivory / cream / shadow | `#FBF6EA` / `#EDE0C3` / `#D3BD94` | bulb |
| garlic root | `#A9845A` | root plate, rootlets |
| garlic rose | `#C68C9B` | thin seam streaks only |

## Accent rule

The interface is neutral, like Safari, Chrome and Firefox. Green is only the accent.

- **Neutral:** page backgrounds, surfaces, sidebars, cards, the toolbar and the tab strip. Body text, borders and dividers. Selected rows and hovered items (`--color-selected`). Bookmark tiles, their letter chips and the tab icons. The lines of the hop diagram.
- **Green (`--color-accent`):** the primary button, the marker of the active or selected item (the bar on the current sidebar link, a pressed toolbar button), the focus ring, links, the I2P badge in the address bar, the dots of the hop diagram, and the bandwidth line in charts.
- **Status colors:** `--color-success` (the "Ready" dot), `--color-warning` and `--color-danger` mark state. A status chip is neutral, with a colored dot.
- No green-tinted background, border or text sits on neutral content.

Added in [#51](https://github.com/tcivie/eepview/pull/51).

## Color rule

eepview uses only the colors on this page.

- **One source.** The token block in `src/ui/theme.css` is the only place that sets a color. Every other CSS, HTML and TS file uses `var(--token)`. Only `transparent`, `currentColor` and `inherit` are allowed outside that block.
- **One list.** `scripts/palette.txt` lists every allowed hex. Each one is named in the tables below.
- **One check.** `scripts/palette-check.sh` fails on:
  - a raw color in `src/` outside `theme.css` (hex, `rgb()`, `hsl()`, `oklch()`, `color-mix()`, a named color, or a color set from TS);
  - a `theme.css` value that is not in the list;
  - an SVG fill, stroke or stop color in `assets/brand/` or `src/ui/assets/` that is not in the list.

  lefthook runs it before each commit, and the `biome + tsc` CI job runs it on each PR.
- **Alpha.** A token may add an alpha byte to a listed hex. For example, `#1c1c1c6b` is `gray-900` at 42% for the dialog scrim. The check compares only the first six digits.

To add a color, add it to `scripts/palette.txt` and name it in a table here, in the same PR.

## Neutral colors

A pure gray ramp: red, green and blue are equal, so no surface leans green.

| Name | Hex | Use |
| --- | --- | --- |
| white | `#FFFFFF` | Surfaces in light, text on the accent in light, highlights in shadows and in the mark |
| gray-50 | `#F6F6F6` | Page background in light |
| gray-100 | `#ECECEC` | Tab strip and wells in light, body text in dark |
| gray-150 | `#E6E6E6` | Selected and hovered items in light |
| gray-200 | `#DCDCDC` | Borders in light |
| gray-400 | `#B3B3B3` | Secondary text in dark |
| gray-450 | `#9A9A9A` | Hints in dark |
| gray-500 | `#8C8C8C` | Control borders in light |
| gray-600 | `#6B6B6B` | Control borders in dark |
| gray-650 | `#5E5E5E` | Hints in light |
| gray-700 | `#4D4D4D` | Secondary text in light |
| gray-800 | `#333333` | Borders in dark |
| gray-825 | `#2E2E2E` | Selected and hovered items in dark |
| gray-850 | `#262626` | Popovers and menus in dark |
| gray-875 | `#1E1E1E` | Surfaces in dark |
| gray-900 | `#1C1C1C` | Body text in light, the scrim and shadows in light |
| gray-925 | `#161616` | Page background in dark |
| gray-950 | `#111111` | Tab strip and wells in dark, text on the accent in dark |
| black | `#000000` | Shadows and the scrim in dark, the mark's shadow |

## Color tokens

Defined in `src/ui/theme.css`. "Brand" marks a value that comes straight from the palette above.

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `--color-bg` | `#F6F6F6` gray-50 | `#161616` gray-925 | Page background, hovered tab |
| `--color-surface` | `#FFFFFF` white | `#1E1E1E` gray-875 | Panels, cards, the nav row |
| `--color-surface-sunken` | `#ECECEC` gray-100 | `#111111` gray-950 | Tab strip, wells, progress tracks |
| `--color-surface-raised` | `#FFFFFF` white | `#262626` gray-850 | Popovers, menus, dialogs |
| `--color-selected` | `#E6E6E6` gray-150 | `#2E2E2E` gray-825 | Selected and hovered items, letter chips |
| `--color-border` | `#DCDCDC` gray-200 | `#333333` gray-800 | Hairlines and panel borders |
| `--color-border-strong` | `#8C8C8C` gray-500 | `#6B6B6B` gray-600 | Control borders, hop lines, high-contrast borders |
| `--color-text` | `#1C1C1C` gray-900 | `#ECECEC` gray-100 | Body text |
| `--color-text-muted` | `#4D4D4D` gray-700 | `#B3B3B3` gray-400 | Secondary text |
| `--color-text-faint` | `#5E5E5E` gray-650 | `#9A9A9A` gray-450 | Hints, axis labels |
| `--color-accent` | `#2A6A56` pine-700 (brand) | `#6CCFA5` accent dark (brand) | See the accent rule |
| `--color-accent-hover` | `#0F3529` pine-900 (brand) | `#93DFBD` mint-200 | Hovered primary button and link |
| `--color-accent-text` | `#FFFFFF` white | `#111111` gray-950 | Text on the accent |
| `--color-accent-soft` | `#D3EADF` mint-100 | `#1A4A3C` pine-750 | The I2P badge, the pulse of a building hop |
| `--color-success` | `#276338` leaf-700 | `#7FCB8F` leaf-300 | The "Ready" dot |
| `--color-warning` | `#875000` amber-700 | `#E6AE5C` amber-300 | Building, outbound bandwidth |
| `--color-warning-soft` | `#F4E3C6` amber-100 | `#3A2B14` amber-900 | Warning boxes |
| `--color-danger` | `#A1252B` brick-700 | `#F2918B` brick-300 | Stopped, refused, errors |
| `--color-danger-soft` | `#F6D9D8` brick-100 | `#3D1C1C` brick-900 | Error boxes |
| `--color-focus` | `#2A6A56` pine-700 (brand) | `#6CCFA5` accent dark (brand) | Focus ring |
| `--color-scrim` | `#1C1C1C6B` gray-900 at 42% | `#00000099` black at 60% | Behind dialogs |
| `--shadow-float` | `#1C1C1C14`, `#1C1C1C24` | `#00000066`, `#00000080` | Popovers and menus |
| `--shadow-inset` | `#FFFFFF99` | `#FFFFFF0A` | The top highlight on panels |

Every text pair passes WCAG AA (4.5:1) in both themes. The lowest pair is `--color-text-faint` on `--color-selected` in dark, at 4.8:1.

## Small and mono variants

All variants are the same artwork. None is redrawn.

- The small variant is the master artwork, scaled to a full-bleed body. It drops the rootlets and the rose streaks, and keeps the same outline, tilt and tones.
- The mono variant is the exact silhouette of the master: the union of its shapes, filled with `currentColor`.
- The neck and the root overlap the bulb, so no background shows through at the joints.

## How to regenerate the icons

1. Render `app-icon-1024.png` from `eepview-mark.svg` with headless Chrome, on a transparent 1024×1024 page.
2. Run `npx tauri icon assets/brand/app-icon-1024.png`. This rewrites `src-tauri/icons/`.
3. Delete the `android/`, `ios/` and `64x64.png` output, because eepview is desktop only.
4. Render `eepview-mark-small.svg` at 1024 px, run `npx tauri icon` on it with `-o` to a temporary folder, and copy `icon.ico`, `32x32.png`, `Square30x30Logo.png` and `Square44x44Logo.png` into `src-tauri/icons/`. The padded macOS icon is blurry at 16–32 px.
5. Check that every PNG is RGBA, and run `cargo build --manifest-path src-tauri/Cargo.toml`.

## Limits

- `src/ui/theme.css` uses the same greens as the mark for its accent tokens (`--color-accent` `#2a6a56` / `#6ccfa5`). Change both together.
- The wordmark is drawn as stroked paths, not as a font.

## History

- 2026-10-03 — Add the logo, the app icons and the brand assets — [#11](https://github.com/tcivie/eepview/pull/11)
- 2026-10-03 — One color source, the palette check — [#38](https://github.com/tcivie/eepview/pull/38)
- 2026-10-03 — Neutral surfaces, green only as the accent — [#51](https://github.com/tcivie/eepview/pull/51)
