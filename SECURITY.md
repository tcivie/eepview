# Security policy

eepview opens I2P sites only. Its core promise is simple. No traffic from the browser leaves through anything but the I2P HTTP proxy. The browser also takes no network action without the consent of the user. A bug that breaks this promise is a security bug.

## Supported versions

eepview has no release yet. Nothing is supported today. After v0.1, only the latest release gets security fixes.

## Report a vulnerability

Report privately through GitHub Security Advisories.

1. Open the [Security tab](https://github.com/tcivie/eepview/security) of the repository.
2. Select "Report a vulnerability". Or go direct to <https://github.com/tcivie/eepview/security/advisories/new>.

Never report a vulnerability in a public issue, a pull request, or a discussion.

Include this information in your report:

- The affected version or commit.
- Your operating system and its WebView engine.
- The steps that show the problem.
- The impact. For example, tell us what data leaves the machine, and where it goes.

## What to expect

- We acknowledge your report within 7 days.
- We send a fix or a plan within 30 days.
- We use coordinated disclosure. We publish the advisory after the fix is ready. We agree the date with you.
- We credit you in the advisory if you want it. Tell us the name to use.

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
- CodeQL scans the code. The repository is public.
- Every GitHub Action is pinned by commit SHA.
- The `main` branch is protected. Changes go through a pull request, and the required checks must pass.
