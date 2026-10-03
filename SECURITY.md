# Security policy

eepview opens I2P sites only. Its core promise is simple. No traffic from the browser leaves through anything but the I2P HTTP proxy. The browser also takes no network action without the consent of the user. A bug that breaks this promise is a security bug.

## Supported versions

eepview has no release yet. Nothing is supported today. After v0.1, only the latest release gets security fixes.

## Report a vulnerability

Report privately. Use one of two channels.

**Channel 1: GitHub private vulnerability reporting.** This is the first channel.

1. Open <https://github.com/tcivie/eepview/security/advisories/new>.
2. Fill in the form and submit it. Only the maintainers can read it.

**Channel 2: email.** Write to gleb@tcivie.com. Start the subject line with "[eepview security]".

Never report a vulnerability in a public issue, a pull request, or a discussion.

Include this information in your report:

- The affected version or commit.
- Your operating system and its WebView engine.
- The steps that show the problem.
- The impact. For example, tell us what data leaves the machine, and where it goes.

## What to expect

- We acknowledge your report within 3 days.
- We triage it within 7 days.
- We send a fix or a plan within 30 days.
- We use coordinated disclosure. We publish the advisory after the fix is ready. We agree the date with you.
- We credit you in the advisory if you want it. Tell us the name to use.

The full steps, from report to advisory and CVE, are in [Vulnerability response](https://github.com/tcivie/eepview/wiki/vulnerability-response).

## Scope

These issues are in scope:

- Any traffic that bypasses the I2P proxy. This includes a clearnet leak, a DNS leak, a WebRTC or UDP leak, and loopback access from a page.
- A page that reaches the internal commands of the app (IPC).
- A consent bypass. This is a network action without a click of the user.
- A change to the router configuration that the user did not approve.
- A supply-chain issue in our build.

These issues are out of scope:

- Bugs in I2P itself. Report them upstream at <https://github.com/i2p/i2p.i2p>.
- Bugs in the WebView engine of the operating system. Report them to the vendor of the engine.
- Differences in browser fingerprints between operating systems. This is a documented limit.

## How we keep the project secure

- Socket.dev reviews the dependencies in every pull request.
- cargo-deny allows crates from crates.io only. It also blocks wildcard versions.
- gitleaks scans the code and the full history for secrets.
- zizmor checks the GitHub workflows.
- CodeQL, OpenSSF Scorecard and dependency review run on GitHub Actions.
- Every GitHub Action is pinned by commit SHA.
- The `main` branch is protected. Changes go through a pull request, and the required checks must pass.
