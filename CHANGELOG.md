# Changelog

git-cliff generates this changelog from the Conventional Commit titles on `main`. The release job writes the release notes from the same titles. Do not edit it by hand. See [Release pipeline](docs/wiki/release-pipeline.md).

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). This project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **ui**: Design system, theme switching, internal pages ([#19](https://github.com/tcivie/eepview/pull/19))
- **ui**: Setup flow polish and router panel ([#27](https://github.com/tcivie/eepview/pull/27))
- **ui**: Use only the brand palette, enforced by palette-check ([#38](https://github.com/tcivie/eepview/pull/38))
- **brand**: Eepview garlic bulb logo and app icons ([#11](https://github.com/tcivie/eepview/pull/11))

### Fixed

- **ui**: Keep the macOS traffic lights off the first tab ([#23](https://github.com/tcivie/eepview/pull/23))
- **ci**: Count PR-only checks in the release gate ([#39](https://github.com/tcivie/eepview/pull/39))
- **ui**: Tab strip inset from chrome_insets, aligned with the back button ([#43](https://github.com/tcivie/eepview/pull/43))

### Documentation

- Add CI, lint, security and license badges to the README ([#4](https://github.com/tcivie/eepview/pull/4))
- Add SECURITY.md and CONTRIBUTING.md ([#7](https://github.com/tcivie/eepview/pull/7))
- Rewrite the README ([#12](https://github.com/tcivie/eepview/pull/12))
- Project wiki, AGENTS.md and roadmap ([#16](https://github.com/tcivie/eepview/pull/16))
- Fix the wiki index and add the PR ownership rule ([#20](https://github.com/tcivie/eepview/pull/20))
- Make the wiki the single home for project docs ([#22](https://github.com/tcivie/eepview/pull/22))
- Release pipeline wiki page ([#21](https://github.com/tcivie/eepview/pull/21))
- Rewrite the README for users ([#36](https://github.com/tcivie/eepview/pull/36))
- User guide and developer guide in the wiki ([#28](https://github.com/tcivie/eepview/pull/28))
- Governance, assurance case, SPDX headers and DCO ([#30](https://github.com/tcivie/eepview/pull/30))
- README pre-release note and roadmap match ([#44](https://github.com/tcivie/eepview/pull/44))

### CI

- Remove the warnings from the CI logs ([#3](https://github.com/tcivie/eepview/pull/3))
- Remove CodeQL and Scorecard until the repo is public ([#9](https://github.com/tcivie/eepview/pull/9))
- Keep main runs from being cancelled; README shows external audits only ([#6](https://github.com/tcivie/eepview/pull/6))
- Release pipeline ([#14](https://github.com/tcivie/eepview/pull/14))
- Unit-test coverage gate for Rust and TypeScript ([#13](https://github.com/tcivie/eepview/pull/13))
- Restore CodeQL and Scorecard, add dependency review and audit badges ([#26](https://github.com/tcivie/eepview/pull/26))
- Release gates without lint exclusions; sign the macOS app in the dmg ([#24](https://github.com/tcivie/eepview/pull/24))
- Signed releases (Sigstore), reproducible Linux builds, hardened release profile ([#37](https://github.com/tcivie/eepview/pull/37))
- Ratchet for the Scorecard and Best Practices scores ([#40](https://github.com/tcivie/eepview/pull/40))
- Run zizmor with the pedantic persona ([#47](https://github.com/tcivie/eepview/pull/47))

### Maintenance

- Repo bootstrap — skeleton, linters, security scans, CI ([#1](https://github.com/tcivie/eepview/pull/1))
- **deps-dev**: Bump typescript from 6.0.3 to 7.0.2 in the npm group ([#2](https://github.com/tcivie/eepview/pull/2))
- Remove every clippy lint exclusion ([#5](https://github.com/tcivie/eepview/pull/5))
- Cut dependencies flagged by Socket ([#10](https://github.com/tcivie/eepview/pull/10))
- Repo hygiene — community files, hash-pinned CI tools, typed vite config ([#15](https://github.com/tcivie/eepview/pull/15))

### Other

- Create FUNDING.yml ([#25](https://github.com/tcivie/eepview/pull/25))

