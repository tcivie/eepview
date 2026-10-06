# Privacy and security

Status: the design is written. The protections are coming in v0.1. eepview is pre-release. Do not use it for anything that needs anonymity yet.

## What eepview protects

- **No traffic outside I2P.** A page can reach the I2P network and nothing else. This holds by design, and a test in CI fails if it breaks.
- **Five layers.** Each one blocks clearnet and local requests on its own. A leak needs all five to fail.
  1. A small proxy inside eepview forwards only `.i2p` hosts. It refuses everything else.
  2. The page view talks only to that proxy.
  3. A filter in the web engine blocks every request that is not for an `.i2p` host.
  4. A navigation guard stops links, pop-ups and redirects to other addresses.
  5. WebRTC is off, so a page cannot send direct UDP traffic.
- **Router checked first.** No page loads until eepview has verified the router. The Home page shows each router check and its real result: the proxy is I2P, the router version, no outproxy, and the network and tunnels. eepview reads the router's tunnel configuration to see an outproxy, and never changes it. An outproxy cannot carry eepview traffic anyway: eepview sends only `.i2p` requests. See [Router checks](router-checks.md).
- **Private by default.** Cookies and cache vanish when you quit. JavaScript is off until you turn it on for a site. You can turn history off.
- **Site icons stay with the site.** Once a day at most, eepview asks the site you visit for its icon (`/favicon.ico`). The request goes to that same `.i2p` site, through the same I2P proxy as the page, with no cookies and no referrer. Nothing goes anywhere else. eepview draws the icon again as a new image and keeps it only while the site is in your bookmarks or history. Clearing history clears the icons too. See [Site icons](site-icons.md).
- **Consent first.** eepview downloads and installs nothing without your click.

See [No-leak architecture](no-leak-architecture.md) for the full design.

## What eepview does NOT protect

- **That you use eepview, from the icon request.** The daily icon request has a fixed, minimal form. A site can tell it apart from the page requests and learn that you use eepview. See [Site icons](site-icons.md).
- **What you type into a site.** If you give a site your name, it knows your name. I2P hides where you are. It does not hide what you say.
- **Files you download.** Downloads are refused for now. When they arrive, a file you open outside eepview can reach the clearnet.
- **Fingerprinting.** eepview uses the web engine of your system. Your engine version, screen size and fonts can still make you recognizable. eepview does not match the uniformity of Tor Browser.
- **A compromised computer.** Malware, a hostile browser extension or a keylogger on your computer sees everything.
- **The I2P network itself.** eepview trusts the I2P release signers, as every I2P router does. A flaw in the I2P protocol is outside eepview.
- **The router's own traffic.** The router talks to the internet, because that is how I2P works. Its first start also contacts reseed servers.

## Do

- Verify downloads (see [Install](install.md)).
- Keep your operating system up to date. eepview uses its web engine.
- Turn on JavaScript only for sites you trust.
- Read the link bubble before you click.

## Do not

- Do not use eepview when your safety depends on anonymity. It is pre-release.
- Do not log in to the same account over eepview and over the clearnet.
- Do not install a file a site tells you to install.
- Do not point eepview at a router you do not trust.

## Report a vulnerability

Report it in private. Never use a public issue. Follow the steps in [SECURITY.md](https://github.com/tcivie/eepview/blob/main/SECURITY.md).

## History

- 2026-10-03 — Write the page — [#28](https://github.com/tcivie/eepview/pull/28)
- 2026-10-03 — Site icons — [#53](https://github.com/tcivie/eepview/pull/53)
