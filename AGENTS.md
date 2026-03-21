# Agent Guidelines for opentui

## Self-Improvement Ledger

- 2026-03-20 | seam: direct Bun JS verification of `syntax-style.test.ts` failed because the platform native package was absent from `packages/core/node_modules/@opentui` | root cause: this checkout had source code but no locally built optional native artifact for `@opentui/core-linux-x64` | canonical fix: run `export PATH="$HOME/.bun/bin:$PATH" && cd packages/core && bun run build:native` before wrapper-level runtime tests | validation seam: `export PATH="$HOME/.bun/bin:$PATH" && cd packages/core && bun test src/syntax-style.test.ts`
- 2026-03-20 | seam: direct Bun JS verification failed because workspace dependencies like `bun-ffi-structs` were not installed locally | root cause: the repo checkout had no populated workspace `node_modules` tree | canonical fix: run `export PATH="$HOME/.bun/bin:$PATH" && bun install` at repo root before JS-side verification | validation seam: `export PATH="$HOME/.bun/bin:$PATH" && bun install && cd packages/core && bun test src/tests/rust-syntax-style-overlay.test.ts`
- 2026-03-20 | seam: Bun-backed package-script verification failed locally because `bun` was unavailable to the current shell | root cause: the environment did not have Bun installed and the installer's PATH update was not visible in the active non-login shell | canonical fix: install Bun to `~/.bun/bin` and prepend `PATH="$HOME/.bun/bin:$PATH"` in verification shells | validation seam: `export PATH="$HOME/.bun/bin:$PATH" && cd packages/core && bun run native:abi:check && bun run native:rust:test`

Default to using Bun instead of Node.js.

- Use `bun <file>` instead of `node <file>` or `ts-node <file>`
- Use `bun test` instead of `jest` or `vitest`
- Use `bun install` instead of `npm install` or `yarn install` or `pnpm install`
- Use `bun run <script>` instead of `npm run <script>` or `yarn run <script>` or `pnpm run <script>`
- Bun automatically loads .env, so don't use dotenv.

NOTE: When only changing typescript, you do NOT need to run the build script.
The build is only needed when changing native code.

## APIs

Don't use bun-specific APIs. Generated code should work in Bun, Node.js and Deno runtimes.

## Testing

Use `bun test` to run tests from the packages directories for a specific package.

```ts#index.test.ts
import { test, expect } from "bun:test";

test("hello world", () => {
  expect(1).toBe(1);
});
```

For more information, read the Bun API docs in `node_modules/bun-types/docs/**.md`.

## Build/Test Commands

To build the project (before running typescript tests), run
`bun run build`
FROM THE REPO ROOT to make sure all packages are built correctly.

To run native tests for `packages/core`, run
`bun run test:native`
FROM THE `packages/core` DIRECTORY.

To filter native tests, use:
`bun run test:native -Dtest-filter="test name"`
FROM THE `packages/core` DIRECTORY.

## Typescript Code Style

- **Runtime**: Bun with TypeScript
- **Formatting**: Prettier (semi: false, printWidth: 120)
- **Imports**: Use explicit imports, group by: built-ins, external deps, internal modules
- **Types**: Strict TypeScript, use interfaces for options/configs, explicit return types for public APIs
- **Naming**: camelCase for variables/functions, PascalCase for classes/interfaces, UPPER_CASE for constants
- **Error Handling**: Use proper Error objects, avoid silent failures
- **Async**: Prefer async/await over Promises, handle errors explicitly
- **Comments**: Minimal comments, NO JSDoc
- **File Structure**: Index files for clean exports, group related functionality
- **Testing**: Bun test framework, descriptive test names, use beforeEach/afterEach for setup

## Debugging

- NOTE this is a terminal UI lib and when running examples or apps built with it,
  you cannot currently see log output like console.log. Ask the user to run the example/app and provide the output.
- Reproduce the issue in a test case. Do NOT start fixing without a reproducible test case.
  Use debug logs to see what is actually happening. DO NOT GUESS.
