#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Tests for scripts/docs-check.sh. Each case builds a small git repo, makes a change on a branch, and runs the check.
# Usage: scripts/docs-check.test.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0
total=0

# A repo with main, a wiki index, one wiki page and a fake changelog generator that prints "generated <revision>".
# The base CHANGELOG.md says "old".
make_repo() {
  rm -rf "$work/repo"
  mkdir -p "$work/repo/scripts" "$work/repo/docs/wiki" "$work/repo/src"
  cd "$work/repo"
  git init -q -b main
  git config user.email test@example.com
  git config user.name test
  cp "$root/scripts/docs-check.sh" scripts/docs-check.sh
  cat > scripts/changelog.sh <<'STUB'
#!/usr/bin/env bash
echo "generated $(git rev-parse "$1")"
STUB
  chmod +x scripts/*.sh
  echo '- [Page](page.md)' > docs/wiki/README.md
  echo page > docs/wiki/page.md
  echo old > CHANGELOG.md
  echo code > src/main.rs
  git add -A
  git commit -q -m base
  git update-ref refs/remotes/origin/main HEAD
  git checkout -q -b change
}

# Move main on by one commit after the PR branch started, as a merge of another PR does.
advance_main() {
  git checkout -q main
  echo more > src/other.rs
  git add -A
  git commit -q -m other
  git update-ref refs/remotes/origin/main HEAD
  git checkout -q change
}

# run_case <name> <expected: pass|fail> <PR title> <file to change> [content] [advance]
# The content "@base" means the generator output for the base tip. "@fork" means the output for the commit where the branch started.
run_case() {
  local name="$1" expected="$2" title="$3" file="$4" content="${5:-changed}" advance="${6:-}"
  local result=pass fork
  make_repo
  fork="$(git rev-parse origin/main)"
  case "$content" in
    @base) content="generated $fork" ;;
    @fork) content="generated $fork" ;;
  esac
  mkdir -p "$(dirname "$file")"
  echo "$content" > "$file"
  git add -A
  git commit -q -m change
  if [ -n "$advance" ]; then
    advance_main
  fi
  if ! BASE_REF=main PR_TITLE="$title" ./scripts/docs-check.sh >/dev/null 2>&1; then
    result=fail
  fi
  total=$((total + 1))
  if [ "$result" = "$expected" ]; then
    echo "ok   $name"
  else
    echo "FAIL $name: expected $expected, got $result"
    failures=$((failures + 1))
  fi
}

# run_title_source <name> <expected> <event|none>: the title comes from the event payload, or from nowhere.
run_title_source() {
  local name="$1" expected="$2" source="$3" result=pass
  make_repo
  echo changed > src/main.rs
  git add -A
  git commit -q -m change
  printf '{"pull_request":{"title":"feat: add a tab"}}' > "$work/event.json"
  if [ "$source" = event ]; then
    export GITHUB_EVENT_PATH="$work/event.json"
  fi
  if ! env -u PR_TITLE BASE_REF=main ./scripts/docs-check.sh >/dev/null 2>&1; then
    result=fail
  fi
  unset GITHUB_EVENT_PATH
  total=$((total + 1))
  if [ "$result" = "$expected" ]; then
    echo "ok   $name"
  else
    echo "FAIL $name: expected $expected, got $result"
    failures=$((failures + 1))
  fi
}

run_case "fix with a code change needs no wiki change" pass "fix: stop the crash" src/main.rs
run_case "ci with a workflow change needs no wiki change" pass "ci: pin the action" .github/workflows/x.yml
run_case "chore with a script change needs no wiki change" pass "chore(deps): bump" scripts/other.sh
run_case "feat without a wiki change fails" fail "feat: add a tab" src/main.rs
run_case "feat with a scope and no wiki change fails" fail "feat(ui): add a tab" src/main.rs
run_case "feat with a breaking mark and no wiki change fails" fail "feat!: drop a flag" src/main.rs
run_case "feat with a scope and a breaking mark fails without a wiki change" fail "feat(ui)!: drop a flag" src/main.rs
run_case "feat with a wiki page change passes" pass "feat: add a tab" docs/wiki/page.md
run_case "a title that only starts with the letters feat is not a feat" pass "feature: add a tab" src/main.rs
run_case "feat that edits only the generated CHANGELOG.md still fails" fail "feat: add a tab" CHANGELOG.md @base
run_case "a hand edit of CHANGELOG.md fails" fail "fix: stop the crash" CHANGELOG.md "- my line"
run_case "CHANGELOG.md equal to the generator output passes" pass "ci: generate the changelog" CHANGELOG.md @base
run_case "a refresh made at the fork point passes after main moved on" pass "ci: refresh the changelog" CHANGELOG.md @fork advance
run_case "a hand edit fails after main moved on" fail "fix: stop the crash" CHANGELOG.md "- my line" advance
run_case "a wiki page missing from the index fails" fail "docs: add a page" docs/wiki/new.md
run_case "a Markdown file outside docs/wiki fails" fail "docs: add notes" notes/todo.md

run_title_source "the title is read from the event payload (feat, no wiki change)" fail event
run_title_source "a missing title fails instead of passing" fail none

echo "$((total - failures)) of $total cases passed"
exit "$failures"
