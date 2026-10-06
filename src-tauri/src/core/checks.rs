// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The four router checks of the Home page (`docs/wiki/router-checks.md`). Pure: the shell
//! gathers the facts, these rules judge them, and the core keeps the result.

use crate::net::outproxy::{OutproxyFinding, RouterKind};
use crate::net::stats::RouterStats;
use crate::types::{CheckId, CheckState, VerifyCheck};

/// The oldest Java I2P release that passes check 2 (V11).
pub const JAVA_MIN_VERSION: (u64, u64, u64) = (2, 4, 0);
/// The oldest i2pd release that passes check 2 (V11).
pub const I2PD_MIN_VERSION: (u64, u64, u64) = (2, 50, 0);
/// Most characters of router text quoted in a detail.
const MAX_QUOTE: usize = 40;

/// One run of a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It passed, with the fact it checked.
    Passed(Option<String>),
    /// It failed: the short reason.
    Failed(String),
    /// eepview cannot check it: the reason.
    NotChecked(String),
}

/// The facts of one VERIFY round (V10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckFacts {
    /// The type of the stored router console, when there is one.
    pub kind: Option<RouterKind>,
    /// The router version: from the helper, else from the console.
    pub version: Option<String>,
    /// What the router configuration says about an outproxy.
    pub outproxy: OutproxyFinding,
    /// The router statistics, when a source answered.
    pub stats: Option<RouterStats>,
}

/// V11: `1.2.3`, `1.2`, `1`, each with an optional `-<text>` suffix. `None` for other text.
#[must_use]
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let numbers = text.trim().split_once('-').map_or(text.trim(), |(v, _)| v);
    let parts: Vec<&str> = numbers.split('.').collect();
    if parts.len() > 3 {
        return None;
    }
    let mut out = [0_u64; 3];
    for (slot, part) in out.iter_mut().zip(&parts) {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse().ok()?;
    }
    Some((out[0], out[1], out[2]))
}

fn dotted((major, minor, patch): (u64, u64, u64)) -> String {
    format!("{major}.{minor}.{patch}")
}

/// Router text for a detail, cut to [`MAX_QUOTE`] characters.
fn quote(text: &str) -> String {
    let mut chars = text.chars();
    let mut out: String = chars.by_ref().take(MAX_QUOTE).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    out
}

/// V11: check 2, the router version against the minimum of its type.
#[must_use]
pub fn version_outcome(kind: Option<RouterKind>, version: Option<&str>) -> Outcome {
    let Some(kind) = kind else {
        return Outcome::NotChecked(
            "No router console was found, so the router type is not known.".into(),
        );
    };
    let Some(text) = version else {
        return Outcome::NotChecked("The router did not report its version.".into());
    };
    let Some(found) = parse_version(text) else {
        return Outcome::NotChecked(format!(
            "The router version \"{}\" is not a version number.",
            quote(text)
        ));
    };
    let (name, min) = match kind {
        RouterKind::Java => ("Java I2P", JAVA_MIN_VERSION),
        RouterKind::I2pd => ("i2pd", I2PD_MIN_VERSION),
    };
    let (shown, least) = (quote(text.trim()), dotted(min));
    if found >= min {
        Outcome::Passed(Some(format!("{name} {shown}, minimum {least}.")))
    } else {
        Outcome::Failed(format!("{name} {shown} is older than the minimum {least}."))
    }
}

/// V12: check 3, no outproxy in the HTTP proxy of the router.
#[must_use]
pub fn outproxy_outcome(finding: &OutproxyFinding) -> Outcome {
    match finding {
        OutproxyFinding::Clear { file } => {
            Outcome::Passed(Some(format!("No outproxy in {}.", quote(file))))
        }
        OutproxyFinding::Listed { file, outproxies } => {
            let noun = if outproxies.len() == 1 {
                "outproxy"
            } else {
                "outproxies"
            };
            let names: Vec<String> = outproxies.iter().map(|o| quote(o)).collect();
            Outcome::Failed(format!(
                "The HTTP proxy in {} lists the {noun} {}.",
                quote(file),
                names.join(", ")
            ))
        }
        OutproxyFinding::Unknown(reason) if reason.trim().is_empty() => {
            Outcome::NotChecked("The router configuration could not be read.".into())
        }
        OutproxyFinding::Unknown(reason) => Outcome::NotChecked(reason.clone()),
    }
}

/// V13: check 4, the network status is OK and a client tunnel is built.
#[must_use]
pub fn tunnels_outcome(stats: Option<&RouterStats>) -> Outcome {
    let Some(stats) = stats else {
        return Outcome::NotChecked("The router statistics are not available.".into());
    };
    let network = stats.network_status.as_deref();
    let client = stats.tunnels.client;
    if let Some(status) = network.filter(|s| !s.eq_ignore_ascii_case("OK")) {
        return Outcome::Failed(format!("Network status: {}.", quote(status)));
    }
    if client == Some(0) {
        return Outcome::Failed("No client tunnel is built yet.".into());
    }
    match (network, client) {
        (Some(_), Some(1)) => Outcome::Passed(Some("Network OK, 1 client tunnel.".into())),
        (Some(_), Some(n)) => Outcome::Passed(Some(format!("Network OK, {n} client tunnels."))),
        (None, _) => Outcome::NotChecked("The router does not report its network status.".into()),
        (Some(_), None) => {
            Outcome::NotChecked("The router does not report its client tunnels.".into())
        }
    }
}

/// The four checks the core keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Checks {
    list: Vec<VerifyCheck>,
}

fn pending(id: CheckId) -> VerifyCheck {
    VerifyCheck {
        id,
        state: CheckState::Pending,
        detail: None,
        passed_at: None,
    }
}

impl Checks {
    /// All four `pending` (V5).
    pub(crate) fn new() -> Self {
        Self {
            list: CheckId::ALL.into_iter().map(pending).collect(),
        }
    }

    pub(crate) fn list(&self) -> &[VerifyCheck] {
        &self.list
    }

    fn get(&self, id: CheckId) -> Option<&VerifyCheck> {
        self.list.iter().find(|c| c.id == id)
    }

    fn get_mut(&mut self, id: CheckId) -> Option<&mut VerifyCheck> {
        self.list.iter_mut().find(|c| c.id == id)
    }

    /// True while check 1 is `passed`.
    pub(crate) fn gate_passed(&self) -> bool {
        self.get(CheckId::ProxyI2p)
            .is_some_and(|c| c.state == CheckState::Passed)
    }

    /// All four back to `pending` (V8).
    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }

    /// V5: a round starts; a `pending` check 1 turns `running`. True when it changed.
    pub(crate) fn start_round(&mut self) -> bool {
        let Some(check) = self.get_mut(CheckId::ProxyI2p) else {
            return false;
        };
        let start = check.state == CheckState::Pending;
        if start {
            check.state = CheckState::Running;
        }
        start
    }

    /// V6, V7, V9: the result of a round. `failure` is `None` when VERIFY passed.
    pub(crate) fn round(&mut self, now: u64, failure: Option<String>) {
        let passed = failure.is_none();
        let outcome = failure.map_or(Outcome::Passed(None), Outcome::Failed);
        self.set(CheckId::ProxyI2p, now, outcome);
        for check in self.list.iter_mut().skip(1) {
            *check = waiting(check, passed);
        }
    }

    /// Sets one check from a run at `now`. True when anything changed (V14).
    pub(crate) fn set(&mut self, id: CheckId, now: u64, outcome: Outcome) -> bool {
        let Some(check) = self.get_mut(id) else {
            return false;
        };
        let next = judged(check, now, outcome);
        let changed = *check != next;
        *check = next;
        changed
    }
}

/// A check 2 to 4 after a round of check 1: `running` once the gate opened (V9), else
/// back to `pending` (V7).
fn waiting(check: &VerifyCheck, gate_open: bool) -> VerifyCheck {
    match (gate_open, check.state) {
        (false, _) => pending(check.id),
        (true, CheckState::Pending) => VerifyCheck {
            state: CheckState::Running,
            ..pending(check.id)
        },
        (true, _) => check.clone(),
    }
}

/// The check after a run: a pass keeps its first pass time (V4).
fn judged(check: &VerifyCheck, now: u64, outcome: Outcome) -> VerifyCheck {
    let (state, detail, passed_at) = match outcome {
        Outcome::Passed(detail) => {
            let since = check
                .passed_at
                .filter(|_| check.state == CheckState::Passed);
            (CheckState::Passed, detail, Some(since.unwrap_or(now)))
        }
        Outcome::Failed(reason) => (CheckState::Failed, Some(reason), None),
        Outcome::NotChecked(reason) => (CheckState::NotChecked, Some(reason), None),
    };
    VerifyCheck {
        id: check.id,
        state,
        detail,
        passed_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_quote_is_cut() {
        let long = "x".repeat(MAX_QUOTE + 5);
        assert_eq!(quote(&long).chars().count(), MAX_QUOTE + 1);
        assert_eq!(quote("2.7.0"), "2.7.0");
    }

    #[test]
    fn an_unknown_id_changes_nothing() {
        let mut checks = Checks { list: Vec::new() };
        assert!(!checks.start_round());
        assert!(!checks.set(CheckId::Version, 1, Outcome::Passed(None)));
        assert!(!checks.gate_passed());
    }
}
