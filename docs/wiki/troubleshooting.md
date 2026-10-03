# Troubleshooting

Status: the pages named here are coming in v0.1.

## Router not found

eepview shows the setup screen when it finds no router.

- Install one from the screen, or install Java I2P yourself and restart eepview.
- If you run a router as a system service, start it first. On Linux that is often `i2p.service`. eepview cannot start a service for you.
- If eepview says a port answers but is not I2P, another program uses that port. eepview refuses to use it.
- If the router is too old, update it, or let eepview install its own next to it.
- If a download is blocked, use "I have the installer file".

## Website Unknown

The name is not in your address book. An address book maps a name like `foo.i2p` to a long address.

- Check the spelling.
- Use a jump service. Open a service such as `reg.i2p` or `stats.i2p` (both are in your bookmarks), and look the name up.
- Or use the long `.b32.i2p` address of the site. It always works.

## Website Unreachable

The name is known, but the site did not answer.

- The site may be offline. Many eepsites run on home computers.
- Your tunnels may still be building. Wait a few minutes.
- Wait, then reload.

## Slow first load

The first visit to a site builds new tunnels. That takes time. The next page loads faster. After a restart the router also needs a minute to warm up. Slowness is normal on I2P.

## Blocked page for a clearnet link

eepview opens only `.i2p` addresses. A link to any other address shows the blocked page. This is by design, and no setting turns it off. Use another browser for that address.

## The router stopped

eepview closes all pages and retries on its own. Use Restart now to try at once. If it keeps failing, collect the logs.

## Collect logs

Include logs when you open an issue. Remove any private data first.

- Run eepview from a terminal to see its output.
- Copy the text of the router status panel.
- Open an issue at [github.com/tcivie/eepview/issues](https://github.com/tcivie/eepview/issues). Do not report vulnerabilities there. See [SECURITY.md](https://github.com/tcivie/eepview/blob/main/SECURITY.md).

## History

- 2026-10-03 — Write the page — [#28](https://github.com/tcivie/eepview/pull/28)
