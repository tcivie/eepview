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
| `preview.html`, `preview-light.png`, `preview-dark.png` | Preview sheet at every size, plus a Dock and a taskbar mock |
| `concepts/*.svg` | The three early concept sketches |

## Palette

| Token | Hex | Use |
| --- | --- | --- |
| pine-700 | `#2A6A56` | icon body top |
| pine-900 | `#0F3529` | icon body bottom |
| pine-950 | `#0D2A23` | dark surfaces |
| text-on-light | `#12302A` | wordmark on light |
| paper | `#F3EDE0` | light surface |
| text-on-dark | `#EEF2EA` | wordmark on dark |
| accent (light theme) | `#23896A` | i-dot, links, focus |
| accent (dark theme) | `#6CCFA5` | i-dot, links, focus |
| garlic ivory / cream / shadow | `#FBF6EA` / `#EDE0C3` / `#D3BD94` | bulb |
| garlic root | `#A9845A` | root plate, rootlets |
| garlic rose | `#C68C9B` | thin seam streaks only |

## Small and mono variants

- The small variant is the bulb silhouette on green, with three tonal cloves, a pointed neck and a root block. Every coordinate is a multiple of 64, so each edge lands on a whole pixel at 16 px and at 32 px.
- The mono variant is the bulb silhouette only, on the same grid, filled with `currentColor`.
- The neck and the root overlap the bulb, so no background shows through at the joints.

## How to regenerate the icons

1. Render `app-icon-1024.png` from `eepview-mark.svg` with headless Chrome, on a transparent 1024×1024 page.
2. Run `npx tauri icon assets/brand/app-icon-1024.png`. This rewrites `src-tauri/icons/`.
3. Delete the `android/`, `ios/` and `64x64.png` output, because eepview is desktop only.
4. Render `eepview-mark-small.svg` at 1024 px, run `npx tauri icon` on it with `-o` to a temporary folder, and copy `icon.ico`, `32x32.png`, `Square30x30Logo.png` and `Square44x44Logo.png` into `src-tauri/icons/`. The padded macOS icon is blurry at 16–32 px.
5. Check that every PNG is RGBA, and run `cargo build --manifest-path src-tauri/Cargo.toml`.

## Limits

- The palette is a proposal for the UI. `src/ui/theme.css` can adopt it.
- The wordmark is drawn as stroked paths, not as a font.

## History

- 2026-10-03 — Add the logo, the app icons and the brand assets — [#11](https://github.com/tcivie/eepview/pull/11)
