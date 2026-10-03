# Browser UI

Status: shipped in [#19](https://github.com/tcivie/eepview/pull/19). The setup flow and the router panel are being added in the `feat/ui-setup-and-router-panel` branch.

The browser chrome and the internal pages live in `src/ui/`. They are plain HTML, CSS and TypeScript, with no framework, and every page passes the CSP `script-src 'self'; style-src 'self'`. Open `src/ui/index.html` with `?dev=1` (run `npm run dev` first) to see every page, filled by the dev mock backend.

## Pages

| Page | File | What it shows |
| --- | --- | --- |
| Toolbar | `toolbar.html` | Tabs, back, forward, reload, home, the address bar with suggestions, the find bar, the JavaScript toggle, the router status dot and the menu. |
| Router panel | `toolbar.html` | A popover opened from the status dot. |
| Setup | `setup.html` | The first-start flow. |
| Home, History, Bookmarks, Network, Settings | `home.html` and the others | The internal pages. |
| Blocked, Router stopped | `blocked.html`, `router-down.html` | Error pages. |
| Status bubble | `status.html` | The link target shown on hover. |

## Setup flow

Every setup screen has the same layout:

- The header shows "Step N of 4" above the hop strip (Router, Download, Connect, Ready).
- The h1 uses one size on every screen.
- One sentence says what is happening. A "What does this mean?" disclosure holds the reason.
- The action bar sticks to the bottom of the window. The one primary action is always at its right.

| Step | Screen | Primary action |
| --- | --- | --- |
| 1 | No router found (`?step=install`) | Accept and download |
| 1 | Router found (`?step=found`) | Use this router |
| 2 | Download (`?step=download`): MB and % per file and in total | Continue, enabled when every signature matches |
| 2 | Download failed (`?step=download-failed`) | Try again |
| 3 | Building tunnels (`?step=tunnels`): tunnels built, hops built, routers known, time | Continue, enabled when both tunnels are built |
| 3 | Tunnels stalled (`?step=tunnels-failed`) | Keep trying |
| 4 | Ready (`?step=ready`) | Open eepview |

The error screens use the parts of the router-stopped page: a red-edged box with a chip, one line of numbers, and the last lines of the log behind a disclosure. The failed step's hop turns red in the hop strip.

Screenshots of every screen before and after the review, in light and dark, at 800×600 and 1440×900: `docs/images/ui/setup-review/`.

## Router panel

A click on the router status dot opens a panel under the dot. The panel shows:

- the state, with a colored dot and one line of explanation;
- the router version, the uptime, the active peers, the tunnels (in, out and participating) and the build success rate;
- the bandwidth now, with a sparkline of the last 10 minutes;
- the proxy address;
- these buttons: "Pause I2P browsing" or "Resume I2P browsing", "Restart router", "Stop router", and "Open Network page".

Restart and Stop are off for a router that eepview does not manage. Their tooltip then reads "Only for a router that eepview manages".

The panel closes on Esc (focus goes back to the dot), on a click outside, and when focus leaves it. While the panel is open, it refreshes every 5 s and the toolbar calls `chrome_set_height` so the panel fits. A missing figure shows as "—".

Screenshots: `docs/images/ui/router-panel-{light,dark}.png`, `router-panel-unmanaged-*.png` and `router-panel-paused-*.png`.

## IPC contract

`src/ui/contract.ts` holds every command and event that the UI uses. Contract v1.2 adds the following, and the shell must confirm each name:

- `RouterStatus.managed` and `RouterStatus.paused` (both boolean);
- `RouterStats.history` (`{ stepSeconds, inBps[], outBps[] }`, which replaces `bandwidthHistory`), `activePeers`, `inboundTunnels` and `outboundTunnels`;
- `connection_pause()`, `connection_resume()` and `router_control({ action: "restart" | "stop" })`.

The toolbar also uses `chrome_insets() -> { left }` and the `chrome-insets-changed` event. `left` is the space the window controls need (about 86 px on macOS, 0 elsewhere). `theme-boot.js` sets an 86 px fallback on macOS before the first paint, so the tab strip does not jump. The first tab never sits left of the back button's edge, so the two line up on Windows and Linux.

Until the shell implements them, the dev mock (`src/ui/mock.ts`) answers these commands. Add `?managed=0` or `?paused=1` to a page URL to preview those states.

## Checks

`npx biome ci .`, `npm run typecheck`, `./scripts/complexity.sh` and `npm run build` must pass. The unit tests for the pure parts live next to them (`src/ui/lib/*.test.ts`) and run with `node --test`.
