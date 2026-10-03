#!/usr/bin/env bash
# Publish docs/wiki to the GitHub wiki. Needs GH_TOKEN and REPO (owner/name).
# README.md becomes Home.md. Relative links are rewritten for the wiki.
set -euo pipefail

cd "$(dirname "$0")/.."
: "${GH_TOKEN:?GH_TOKEN must be set}"
: "${REPO:?REPO must be set}"

src="$PWD/docs/wiki"
blob="https://github.com/${REPO}/blob/main"
git clone --quiet "https://x-access-token:${GH_TOKEN}@github.com/${REPO}.wiki.git" wiki

cp "$src"/*.md wiki/
mv wiki/README.md wiki/Home.md
sed -i \
  -e "s#](\.\./\.\./#](${blob}/#g" \
  -e "s#](\.\./#](${blob}/docs/#g" \
  -e 's#\](\([a-z0-9-]*\)\.md)#](\1)#g' \
  wiki/*.md
echo "Generated from docs/wiki in the repository. Edit there, not here." > wiki/_Footer.md

cd wiki
git add -A
if git diff --cached --quiet; then
  echo "Wiki is up to date."
  exit 0
fi
git -c user.name="github-actions[bot]" \
  -c user.email="41898282+github-actions[bot]@users.noreply.github.com" \
  commit --quiet -m "docs: sync from docs/wiki"
git push --quiet origin HEAD
