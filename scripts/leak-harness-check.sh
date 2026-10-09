#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Lints and tests the leak harness (tests/leak): ruff check, ruff format and the unit tests.
# ruff is pinned by hash in tests/leak/requirements-lint.txt. Both CI lanes call this script.
# Usage: scripts/leak-harness-check.sh   (the venv goes to RUNNER_TEMP, or TMPDIR, or /tmp)
set -euo pipefail

venv="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/ruff-venv"
export PIP_DISABLE_PIP_VERSION_CHECK=1
python3 -m venv "$venv"
"$venv/bin/pip" install --quiet --no-deps --require-hashes -r tests/leak/requirements-lint.txt
"$venv/bin/ruff" check tests/leak
"$venv/bin/ruff" format --check tests/leak
python3 -m unittest discover -s tests/leak -v
