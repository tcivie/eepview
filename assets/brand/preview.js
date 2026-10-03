// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

const THEMES = ["light", "dark"];

function applyTheme() {
  const theme = location.hash.slice(1);
  if (THEMES.includes(theme)) document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
}

applyTheme();
addEventListener("hashchange", applyTheme);
