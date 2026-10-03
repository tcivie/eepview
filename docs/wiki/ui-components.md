# UI components

Status: shipped in [#65](https://github.com/tcivie/eepview/pull/65).

Every internal page and the toolbar use one shared set of components. A page does not restyle a button, a card or a list row. It uses the shared class, and it keeps only the layout that no other page has.

## Stylesheets

The stylesheets are split into cascade layers. A later layer always wins, so a page rule never has to fight a component rule on specificity.

| File | Layer | Holds |
| --- | --- | --- |
| `src/ui/theme.css` | none | The tokens: colors, spacing, radii, font sizes, component sizes, motion. It also declares the layer order. |
| `src/ui/ui.css` | `base` | The reset, headings, focus ring. |
| `src/ui/ui.css` | `components` | The shared components below. |
| `src/ui/pages.css` | `pages` | Layout that only one internal page has. |
| `src/ui/chrome.css` | `pages` | Layout that only the toolbar and the link status bubble have. |
| `src/ui/ui.css` | `state` | `[hidden]` hides an element, whatever `display` a component sets. |

## Components

| Component | Classes | Used on |
| --- | --- | --- |
| Page header | `.page-head`, `.display` (large title), `.lede` (the sentence under it), `.section`, `.section-head`, `.actions` | Every page |
| Card | `.card`, `.card-pad`, `.card-flush`, `.card-link` (a card that is a link), `.card-title` | Home, Bookmarks, History, Network, Settings, Setup |
| Floating surface | `.popover`, `.dialog`, `.page-toast` | Menus, suggestions, tooltip, router panel, toasts, the bookmark editor |
| Stat grid | `.facts`, `.facts-lg` | Home, Network, Settings, Setup, router panel |
| Button | `.btn` (secondary), `.btn-primary`, `.btn-ghost`, `.btn-danger`, `.btn-danger-solid`, `.btn-small`, `.btn-large` | Every page |
| Icon button | `.icon-btn`, `.icon-btn-sm`, `.icon-btn-text`, `.icon-btn-danger` | Toolbar, Bookmarks, History |
| Link | `a`, `.link` (a link-styled element that is not an anchor) | Every page |
| Input | `.field`, `.field-label`, `.field-error`, `.input`, `.input-sm`, `.select`, `.input-icon`, `.check`, `.choices`, `.segmented`, `.switch` | Settings, Bookmarks, History, Setup, find bar |
| List row | `.list`, `.list-row`, `.row-link`, `.row-title`, `.row-actions`, `.addr` | Bookmarks, History, Home tiles |
| Empty state | `.empty` | Home, History |
| Banner | `.banner`, `.banner-lg`, `data-tone="warning"` or `"danger"` | History, Settings, Setup, Blocked, Router stopped |
| Status dot | `.dot` in a parent with `data-tone`, and `.chip::before` | Toolbar, router panel, chips |
| Chip | `.chip` with `data-tone="ready"`, `"building"` or `"stopped"` | Home, Network, Setup, Router stopped |
| Letter icon | `.letter-icon`, `.letter-icon-lg`, `.letter-icon-add` | Tabs, Home tiles |
| Step indicator | `.hops`, `.hop`, `.hop-node`, `.hop-end`, `data-state`, `aria-current="step"`, `.step-count` | Setup, Home, Blocked, Router stopped |
| Table | `.table` | Settings, Setup |
| Sparkline | `.spark-area`, `.spark-line`, `.spark-in`, `.spark-out`, `.spark-axis` | Network, router panel |
| Log | `.log`, `.log-lines` | Setup, Router stopped |
| Text | `.hint` (small, muted), `.caption` (extra small, muted), `.muted`, `.mono`, `.num`, `.truncate`, `.visually-hidden` | Every page |
| App shell | `.shell`, `.rail`, `.rail-nav`, `.content`, `.brand`, `.mark` | Home, Bookmarks, History, Network, Settings |

## The style check

`scripts/style-check.sh` keeps the set shared. It runs in lefthook and in the `biome + tsc` lint job. It has no exclusions. It fails when:

- a stylesheet sets `margin`, `padding`, `gap`, a `border-radius`, `font-size` or `font` to a raw length (such as `12px`, `0.5rem` or `50%`) instead of a token. `0` is allowed, and so is `calc()` of tokens and plain numbers. A raw fallback inside `var()` fails too.
- a stylesheet other than `theme.css` defines a custom property. Tokens live in one file.
- an HTML file has a `style` attribute or a `<style>` element.
- a TypeScript file, or a script under `scripts/`, sets an inline style (`.style`, `cssText`, `setAttribute("style", …)`).

`scripts/palette-check.sh` covers raw colors.

The toolbar gets the width of the macOS window buttons at run time. `src/ui/toolbar/layout.ts` writes it to `--chrome-inset-left` in a constructed stylesheet, not in an inline style.

## Screenshots

`scripts/screenshots.sh` takes every page in light and dark (see [Brand](brand.md#screenshots)). The images are in `docs/images/ui/`.

## History

- 2026-10-03 — One shared component set, cascade layers and the style check — [#65](https://github.com/tcivie/eepview/pull/65)
