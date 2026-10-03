# Brand

Status: in progress.

The eepview logo and app icons.

Work: [PR #11](https://github.com/tcivie/eepview/pull/11)

Page is completed by the PR that ships the feature.

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

## Color rule

eepview uses only the colors on this page.

- **One source.** The token block in `src/ui/theme.css` is the only place that sets a color. Every other CSS, HTML and TS file uses `var(--token)`. Only `transparent`, `currentColor` and `inherit` are allowed outside that block.
- **One list.** `scripts/palette.txt` lists every allowed hex. Each one is named in the tables below.
- **One check.** `scripts/palette-check.sh` fails on:
  - a raw color in `src/` outside `theme.css` (hex, `rgb()`, `hsl()`, `oklch()`, `color-mix()`, a named color, or a color set from TS);
  - a `theme.css` value that is not in the list;
  - an SVG fill, stroke or stop color in `assets/brand/` or `src/ui/assets/` that is not in the list.

  lefthook runs it before each commit, and the `biome + tsc` CI job runs it on each PR.
- **Alpha.** A token may add an alpha byte to a listed hex. For example, `#12302a6b` is `text-on-light` at 42% for the dialog scrim. The check compares only the first six digits.

To add a color, add it to `scripts/palette.txt` and name it in a table here, in the same PR.

## Neutral colors

| Name | Hex | Use |
| --- | --- | --- |
| white | `#FFFFFF` | Raised surfaces in light, text on the accent in light, highlights in shadows and in the mark |
| black | `#000000` | Shadows and the scrim in dark, the mark's shadow |

## Color tokens

Defined in `src/ui/theme.css`. "Brand" marks a value that comes straight from the palette above.

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `--color-bg` | `#F3EDE0` paper (brand) | `#0D2A23` pine-950 (brand) | Page background, hovered tab |
| `--color-surface` | `#FAF7F0` paper-50 | `#12342B` pine-900-surface | Panels, the nav row |
| `--color-surface-sunken` | `#E8E1D2` paper-200 | `#0A211B` pine-975 | Tab strip, wells, progress tracks |
| `--color-surface-raised` | `#FFFFFF` white | `#173E33` pine-850 | Popovers, menus, dialogs |
| `--color-border` | `#D3CAB8` paper-300 | `#255044` pine-800 | Hairlines and panel borders |
| `--color-border-strong` | `#8F8A7C` paper-500 | `#5A8576` pine-500 | Control borders, high-contrast borders |
| `--color-text` | `#12302A` text-on-light (brand) | `#EEF2EA` text-on-dark (brand) | Body text |
| `--color-text-muted` | `#45574F` ink-600 | `#ABC0B6` mist-300 | Secondary text |
| `--color-text-faint` | `#56685F` ink-500 | `#97ADA3` mist-400 | Hints, axis labels |
| `--color-accent` | `#2A6A56` pine-700 (brand) | `#6CCFA5` accent dark (brand) | Links, primary buttons, the active state |
| `--color-accent-hover` | `#0F3529` pine-900 (brand) | `#93DFBD` mint-200 | Hovered primary |
| `--color-accent-text` | `#FFFFFF` white | `#0D2A23` pine-950 (brand) | Text on the accent |
| `--color-accent-soft` | `#D3EADF` mint-100 | `#1A4A3C` pine-750 | Selected rows, chart fills |
| `--color-accent-line` | `#23896A` accent light (brand) | `#4FB48A` mint-500 | Built hops, the active tab line |
| `--color-success` | `#276338` leaf-700 | `#7FCB8F` leaf-300 | Router ready |
| `--color-success-soft` | `#D6EADB` leaf-100 | `#1B3423` leaf-900 | Ready chip background |
| `--color-warning` | `#875000` amber-700 | `#E6AE5C` amber-300 | Building, outbound bandwidth |
| `--color-warning-soft` | `#F4E3C6` amber-100 | `#3A2B14` amber-900 | Building chip background |
| `--color-danger` | `#A1252B` brick-700 | `#F2918B` brick-300 | Stopped, refused, errors |
| `--color-danger-soft` | `#F6D9D8` brick-100 | `#3D1C1C` brick-900 | Error box and chip background |
| `--color-focus` | `#2A6A56` pine-700 (brand) | `#6CCFA5` accent dark (brand) | Focus ring |
| `--color-scrim` | `#12302A6B` text-on-light at 42% | `#00000099` black at 60% | Behind dialogs |
| `--shadow-float` | `#12302A14`, `#12302A24` | `#00000066`, `#00000080` | Popovers and menus |
| `--shadow-inset` | `#FFFFFF99` | `#FFFFFF0A` | The top highlight on panels |

Every text pair passes WCAG AA (4.5:1) in both themes.
