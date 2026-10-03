// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The one function of node:module that a test uses: the dev backend imports a JSON file, and
// Node asks for an import attribute that the bundler does not need.
declare module "node:module" {
  type LoadContext = { importAttributes?: Record<string, string> };
  export function registerHooks(hooks: {
    load(
      url: string,
      context: LoadContext,
      nextLoad: (url: string, context?: LoadContext) => unknown,
    ): unknown;
  }): void;
}
