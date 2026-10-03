<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# Links, menus and shortcuts

Status: in progress in [#77](https://github.com/tcivie/eepview/pull/77).

How a click on a link, the right-click menu, the keyboard shortcuts and the extra mouse buttons work. Everything on this page works with page JavaScript off. The engine reports the click, the key or the menu request, and the shell acts on it. No page script takes part.

Security: every tab that this page opens goes through the L4 navigation guard (`nav::guard`), the same as any other tab. Nothing here weakens a leak layer. See [ADR 0001](adr-0001-no-leak-architecture.md).

## Requirements

Each requirement is a test target. The tests check these rules, not the code. "Cmd" is the Command key on macOS. "Ctrl" is the Control key. "The new-tab key" is Cmd on macOS and Ctrl on Windows and Linux.

### Links

- **L1.** A click on a link with the primary button and no modifier opens the link in the same tab, as today.
- **L2.** A click on a link with the new-tab key (Cmd+click on macOS, Ctrl+click on Windows and Linux) opens the link in a new background tab. The new tab shows in the tab strip. The active tab, the address bar text and the keyboard focus do not change. The page in the current tab does not navigate.
- **L3.** A middle-click on a link does the same as L2, on every system.
- **L4.** The new-tab key with Shift, and a middle-click with Shift, open the link in a new foreground tab: the new tab becomes the active tab. The current tab does not navigate.
- **L5.** Shift+click with no new-tab key opens the link in a new foreground tab. (Other browsers open a new window. eepview has one window.)
- **L6.** Alt (Option) has no effect on a link click: Alt+click acts as a plain click. It never starts a download. On macOS, Ctrl+click is a secondary click: it opens the context menu and never follows the link. On Windows and Linux, the Windows (Super) key has no effect on a link click.
- **L7.** A link that has keyboard focus follows the same rules when you press Enter: Enter alone opens it in the same tab, the new-tab key+Enter opens a background tab, and the new-tab key+Shift+Enter opens a foreground tab.
- **L8.** A new tab from a link opens right after the tab that opened it. When the same tab opens several links in a row, each new tab opens after the previous one, so the tabs keep the order of the clicks. The run ends when you select another tab or open a tab in another way (Cmd/Ctrl+T, the + button, a typed address).
- **L9.** Every new tab from L2 to L5 and from the context menu passes `nav::guard` before any tab state changes. A target that is not an `http(s)` URL on a `.i2p` host opens nothing: no tab, no load, no request. The current tab does not navigate either. The same warning toast shows as for a refused new window today.
- **L10.** When L2 to L5 apply, the shell cancels the engine's own navigation. The engine never loads the link in the current tab and never opens a window of its own.
- **L11.** L1 to L10 work with page JavaScript off and on, in web pages (`tab-*`) and in internal pages (Home, Bookmarks, History).
- **L12.** A page's own new-window request (`target=_blank`, `window.open`) with a plain click opens a foreground tab, as today. With the new-tab key or a middle-click, it follows L2 to L4.
- **L13.** A background tab loads its page at once, the same as a foreground tab. Its tab shows the loading state. When the router is down, it behaves as any web tab does today.

### Context menu

- **C1.** A right-click opens the eepview context menu. So do Ctrl+click on macOS, and the Menu key and Shift+F10 on Windows and Linux. This applies in a web page, an internal page, the address bar and the find field. eepview builds the menu. The engine's default menu never shows. The menu works with page JavaScript off.
- **C2.** The menu shows exactly the entries of `context_menu(target)`, in that order, and nothing else. The shell, the engine and the operating system add no item.
- **C3.** In a text field (`editable`): Undo, Redo, separator, Cut, Copy, Paste, separator, Select All. Cut and Copy are enabled only when text is selected. The other items are always enabled. A link or an image under the pointer adds nothing in a text field.
- **C4.** On a link: Open Link in New Tab, Open Link in New Background Tab, Copy Link Address.
- **C5.** On an image: Open Image in New Tab, Copy Image Address, Copy Image.
- **C6.** On selected text (not in a text field): Copy.
- **C7.** When there is no link, no image, no selection and no text field: Back, Forward, Reload, separator, Bookmark This Page, Copy Page Address, separator, Find. Back is enabled only when the tab can go back. Forward is enabled only when the tab can go forward. Bookmark This Page is enabled only when the page is an I2P page that has no bookmark yet. The other items are always enabled.
- **C8.** Several targets at once (for example an image inside a link, with selected text) show their groups in the order link, image, selection, with one separator between two groups. The page group of C7 shows only when no other group shows. A menu never starts or ends with a separator and never has two separators in a row.
- **C9.** "Open Link in New Tab" and "Open Image in New Tab" open a new foreground tab (L4). "Open Link in New Background Tab" opens a new background tab (L2). Each one goes through L9. Each one is enabled only when its target is an `http(s)` URL on a `.i2p` host. For any other target the item shows, disabled.
- **C10.** Copy Link Address, Copy Image Address and Copy Page Address put the absolute URL on the clipboard as plain text. Copy Image puts the image that the page already shows on the clipboard. Copy Image is enabled only when the image URL is an `http(s)` URL on a `.i2p` host.
- **C11.** Back, Forward, Reload, Bookmark This Page and Find do the same as their shortcuts (K1).
- **C12.** In the toolbar, the status bubble and the popups, a right-click outside a text field opens no menu.
- **C13.** Esc, or a click outside the menu, closes the menu and does nothing else.

### Privacy of the menus

- **P1.** No eepview menu (the context menu or the menu bar) has an item that sends data out of eepview, sends it to the clearnet, or hands it to another app. The default engine and system menus have such items, and none of them may appear: "Search with Google" (or any search engine), "Look Up", "Translate", "Share…", "Services", "Open Link" (in the default browser), "Open Link in New Window", "Open in New Window", "Download Linked File", "Download Image", "Save Image As", "Inspect Element", "Writing Tools", "AutoFill", "Speech".
- **P2.** The context menu uses only the 18 item ids of `ItemId::ALL` (see the interface). A test checks that every menu, for every target, holds only these ids. A test checks that no label contains a word of the P1 list ("search", "look up", "translate", "share", "services", "download", "save", "inspect", "window", "writing tools", "autofill", "speech"), and that every label that starts with "Open" ends with "Tab".
- **P3.** The context menu never shows "Inspect Element", in any build.
- **P4.** On macOS, the menu bar has no "Start Dictation…" item and no "Services" submenu. (AppKit adds them by default.)
- **P5.** Opening the context menu, or moving the pointer over its items, starts no network request. Only an "Open …" item makes a request, and only after L9.

### Keyboard

- **K1.** The shortcuts are the rows of this table, and only these. "New" marks a shortcut that this change adds. "Changed" marks a change to a shortcut that exists today.

| Action | macOS | Windows and Linux | Menu item ids | Note |
| --- | --- | --- | --- | --- |
| New tab | Cmd+T | Ctrl+T | `new-tab` | |
| New window (opens a new tab: eepview has one window) | Cmd+N | Ctrl+N | `new-window` | New |
| Close tab | Cmd+W | Ctrl+W, Ctrl+F4 | `close-tab`, `close-tab-f4` | Ctrl+F4 new |
| Reopen closed tab | Cmd+Shift+T | Ctrl+Shift+T | `reopen-tab` | |
| Address bar | Cmd+L | Ctrl+L, Alt+D, F6 | `focus-address`, `focus-address-alt`, `focus-address-f6` | Alt+D and F6 new |
| Reload | Cmd+R | Ctrl+R, F5 | `reload`, `reload-f5` | F5 new |
| Reload without the cache | Cmd+Shift+R | Ctrl+Shift+R, Ctrl+F5 | `hard-reload`, `hard-reload-f5` | Ctrl+F5 new |
| Stop | Esc (see K4), Cmd+. | Esc (see K4) | `stop`, `stop-period` | Cmd+. new; Esc changed |
| Back | Cmd+[ | Ctrl+[, Alt+Left | `back`, `back-alt` | Changed: Option+Left is no longer Back on macOS (K5) |
| Forward | Cmd+] | Ctrl+], Alt+Right | `forward`, `forward-alt` | Changed: Option+Right is no longer Forward on macOS (K5) |
| Next tab | Ctrl+Tab, Cmd+Shift+] | Ctrl+Tab, Ctrl+PageDown | `next-tab`, `next-tab-alt` | Cmd+Shift+] and Ctrl+PageDown new |
| Previous tab | Ctrl+Shift+Tab, Cmd+Shift+[ | Ctrl+Shift+Tab, Ctrl+PageUp | `prev-tab`, `prev-tab-alt` | Cmd+Shift+[ and Ctrl+PageUp new |
| Tab 1 to 8 | Cmd+1 to Cmd+8 | Ctrl+1 to Ctrl+8 | `tab-1` … `tab-8` | |
| Last tab | Cmd+9 | Ctrl+9 | `tab-9` | |
| Find | Cmd+F | Ctrl+F | `open-find` | |
| Find next | Cmd+G | Ctrl+G | `find-next` | |
| Find previous | Cmd+Shift+G | Ctrl+Shift+G | `find-prev` | |
| Bookmark this page | Cmd+D | Ctrl+D | `bookmark` | |
| Bookmarks | Cmd+Shift+B | Ctrl+Shift+B | `bookmarks` | |
| History | Cmd+Y | Ctrl+H | `history` | |
| Zoom in | Cmd+=, Cmd+Shift+= (Cmd++) | Ctrl+=, Ctrl+Shift+= (Ctrl++) | `zoom-in`, `zoom-in-plus` | Shift form new |
| Zoom out | Cmd+- | Ctrl+- | `zoom-out` | |
| Actual size | Cmd+0 | Ctrl+0 | `zoom-reset` | |
| Home | Cmd+Shift+H | Ctrl+Shift+H, Alt+Home | `home`, `home-alt` | Alt+Home new |
| Settings | Cmd+, | Ctrl+, | `settings` | |
| Quit | Cmd+Q (app menu) | none (close the window) | macOS app menu | |

- **K2.** Every shortcut of K1 works while the keyboard focus is in a web page (JavaScript on or off), in the address bar, in the find field, in an internal page, or in an open popup (the menu or the router panel). It also works when no element has focus, and the first time after start, with no click first.
- **K3.** A page cannot take a shortcut of K1. A page's own key handler cannot stop it.
- **K4.** Esc depends on the focus. In a web page or an internal page, Esc stops a page that loads. In the address bar, Esc closes the suggestions. In the find field, Esc closes the find bar. In a popup, Esc closes the popup. Esc in those three places never stops a load. Cmd+. on macOS stops a load wherever the focus is.
- **K5.** Text-editing keys belong to the text field that has focus. None of them is a shortcut: Option+Left and Option+Right (macOS), Cmd+Left and Cmd+Right (macOS), Ctrl+Left and Ctrl+Right (Windows, Linux), Home, End, Backspace, Delete, and Shift with any of them. On macOS, Option+Left and Option+Right move by one word in the address bar and in page text fields.
- **K6.** Backspace and Shift+Backspace never go back or forward, wherever the focus is.
- **K7.** A shortcut that has nothing to do (Back with no history, Find next with no search) does nothing and shows no error.
- **K8.** Cmd/Ctrl+N does exactly what Cmd/Ctrl+T does: a new tab, with focus in the address bar and its text selected (browser shell, UX batch 1, rule 4).
- **K9.** A key with modifiers that do not match a row exactly is not that shortcut. For example, Ctrl+Shift+T is "Reopen closed tab", never "New tab", and Cmd+T on Windows does nothing.

### Mouse

- **B1.** A middle-click on a tab in the tab strip closes that tab, the same as its close button. A middle-click on the empty part of the strip does nothing.
- **B2.** A double-click on the empty part of the tab strip opens a new tab, as Cmd/Ctrl+T does (K8). On macOS the window does not zoom or minimize on that double-click.
- **B3.** The mouse back and forward buttons go back and forward in the active tab. This works wherever the pointer is in the window: a web page (JavaScript on or off), an internal page, or the toolbar.

## Public interface

The tests call these. All of them are pure: no Tauri runtime, no engine.

### Rust, in the `eepview_lib` crate

`input` (new module):

- `Modifiers { meta: bool, ctrl: bool, alt: bool, shift: bool }`, with `Default` (all false). `meta` is the Command key on macOS and the Windows (Super) key on Windows and Linux. `alt` is the Option key on macOS.
- `MouseButton`: `None` (a keyboard activation, such as Enter on a link), `Primary`, `Middle`, `Secondary`, `Back`, `Forward`.
- `Disposition`: `CurrentTab`, `NewBackgroundTab`, `NewForegroundTab`.
- `link_disposition(mac: bool, modifiers: Modifiers, button: MouseButton) -> Disposition`. The new-tab key is `meta` when `mac` is true, and `ctrl` when it is false. The rules (L1 to L7):
  - `Middle`: `NewForegroundTab` with `shift`, else `NewBackgroundTab`. Other modifiers do not matter.
  - `Primary` or `None` with the new-tab key: `NewForegroundTab` with `shift`, else `NewBackgroundTab`.
  - `Primary` or `None` without the new-tab key: `NewForegroundTab` with `shift`, else `CurrentTab`.
  - `Secondary`, `Back`, `Forward`: `CurrentTab`. (These buttons never follow a link. The shell opens no tab for them.)
  - `alt` never changes the result. Neither does the key that is not the new-tab key (`ctrl` on macOS, `meta` elsewhere).

`shortcuts` (existing module, extended):

- `table(mac: bool) -> Vec<Shortcut>`: the rows of K1 for one system, `mac` true for macOS. The Quit row is not in it: Quit is the macOS app menu item. Each row has `id`, `label`, `accel` and `menu`, as today. `accel` uses the Tauri accelerator syntax with `CmdOrCtrl`, `Ctrl`, `Alt`, `Shift` and the key names of `KeyboardEvent.code` (`KeyT`, `Digit1`, `BracketLeft`, `ArrowLeft`, `PageDown`, `F5`, `Escape`, `Comma`, `Period`, `Equal`, `Minus`, `Tab`, `Home`). Ids are unique, and accelerators are unique, in each table.
- `action(id: &str) -> Option<Action>`: as today, plus the new ids of K1. `new-window` gives `Action::NewTab`. Each `*-alt`, `*-f4`, `*-f5`, `*-f6`, `*-plus` and `*-period` id gives the action of its row.
- `Chord { code: String, modifiers: input::Modifiers }`: one key press. `code` is a `KeyboardEvent.code` name.
- `lookup(mac: bool, chord: &Chord) -> Option<Action>`: the action of the K1 row whose keys are exactly `chord` on that system, or `None`. `CmdOrCtrl` means `meta` on macOS and `ctrl` elsewhere. All four modifiers must match: an extra or a missing modifier gives `None` (K9). Every text-editing key of K5 and every Backspace chord (K6) gives `None`.
- `mouse_action(button: input::MouseButton) -> Option<Action>`: `Back` gives `Action::Back`, `Forward` gives `Action::Forward`, any other button gives `None` (B3).

`context_menu` (new module):

- `ItemId`: `OpenLinkInNewTab`, `OpenLinkInBackgroundTab`, `CopyLinkAddress`, `OpenImageInNewTab`, `CopyImageAddress`, `CopyImage`, `Undo`, `Redo`, `Cut`, `Copy`, `Paste`, `SelectAll`, `Back`, `Forward`, `Reload`, `BookmarkPage`, `CopyPageAddress`, `Find`.
  - `ItemId::ALL: [ItemId; 18]`: every id, once.
  - `ItemId::as_str(self) -> &'static str`: `open-link-new-tab`, `open-link-background-tab`, `copy-link-address`, `open-image-new-tab`, `copy-image-address`, `copy-image`, `undo`, `redo`, `cut`, `copy`, `paste`, `select-all`, `back`, `forward`, `reload`, `bookmark-page`, `copy-page-address`, `find`.
  - `ItemId::label(self) -> &'static str`: "Open Link in New Tab", "Open Link in New Background Tab", "Copy Link Address", "Open Image in New Tab", "Copy Image Address", "Copy Image", "Undo", "Redo", "Cut", "Copy", "Paste", "Select All", "Back", "Forward", "Reload", "Bookmark This Page", "Copy Page Address", "Find".
- `Entry`: `Item { id: ItemId, enabled: bool }` or `Separator`.
- `Target` (`Default`: no link, no image, no selection, not editable, empty page, cannot go back or forward, not bookmarked): `link: Option<String>` (the absolute URL of the link under the pointer), `image: Option<String>` (the absolute URL of the image under the pointer), `selection: bool` (text is selected), `editable: bool` (the pointer is in a text field), `page: String` (the URL of the page), `can_back: bool`, `can_forward: bool`, `bookmarked: bool` (the page has a bookmark).
- `context_menu(target: &Target) -> Vec<Entry>`: the menu of C3 to C9, in order. "An I2P target" is a URL for which `nav::is_allowed` is true.

`core::Core` (existing, extended):

- `open_link(&mut self, url: &Url, how: input::Disposition) -> Vec<Effect>`: opens a link from a page or from the context menu.
  - A URL that `nav::is_allowed_web` refuses: no tab changes, the active tab does not change and does not navigate, and the effects are one warning toast (`Event::Toast` with kind `warn`), as `new_window` gives today (L9).
  - `CurrentTab`: the active tab navigates to `url`, as for a link click today.
  - `NewBackgroundTab`: a new tab with `url`, placed by L8. The active tab stays the same. The effects hold `Emit(TabsChanged)` and the load of the new tab, and no `FocusToolbar` and no `FocusContent` (L2, L13).
  - `NewForegroundTab`: a new tab with `url`, placed by L8, and it becomes the active tab (L4).
- `new_window(&mut self, url: &Url) -> Vec<Effect>`: unchanged, a foreground tab (L12).

### TypeScript (pure)

`src/ui/lib/tab-strip.ts` (existing, extended):

- `stripMouse(on: "tab" | "empty", button: number, clicks: number): "close-tab" | "new-tab" | null`. `button` is `MouseEvent.button` (0 primary, 1 middle, 2 secondary, 3 back, 4 forward). `clicks` is `MouseEvent.detail`. A middle button on a tab gives `"close-tab"` (B1). The primary button with 2 clicks on the empty strip gives `"new-tab"` (B2). Anything else gives `null`.

### Platform bridge (not unit-tested)

The `eepview-platform` crate holds the engine glue for each system, the only place with `unsafe` code (ADR 0001). It reports a link activation (URL, modifiers, button), a context menu request (the `Target`), a key press (`Chord`, where the menu bar cannot see it), and a mouse back or forward button. The shell turns them into `link_disposition`, `context_menu`, `lookup` and `mouse_action`, and calls the core. Manual QA covers it on macOS, Windows and Linux.

## History

- 2026-10-03 — Requirements for link clicks, the context menu, keyboard shortcuts and mouse buttons — [#77](https://github.com/tcivie/eepview/pull/77)
