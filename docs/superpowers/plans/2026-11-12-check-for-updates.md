# Check for Updates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Add CloneFleet's manual release-check flow to Marduk Help.

**Architecture:** A frontend release helper fetches and validates GitHub release metadata and compares semantic versions. A small Help component renders localized states and uses the existing external-link handler for downloads.

**Tech Stack:** React, TypeScript, Vitest, Tauri v2.

**Spec:** `docs/superpowers/specs/2026-11-12-check-for-updates-design.md`

## Global Constraints

- Only clicking the button contacts GitHub's latest-release endpoint for `keeganwitt/marduk-agent-sessions`.
- No background checks, persisted preferences, installer downloads, or automatic installation.
- Localize the new interface text using the existing catalogs.
- Permit the GitHub API request in the production content security policy.

## Review Focus

- Prerelease and numeric version ordering must not claim an older version is newer.
- Invalid release data and unexpected download URLs must produce retryable errors.
- Slow requests must time out and re-enable the check button.
- An unavailable installed version must never produce an up-to-date result.
- Errors must clear on retry; no network requests should run merely from opening Help.

### Task 1: Release helper

**Files:** Create `src/lib/updates.ts` and `src/lib/updates.test.ts`.

**Interfaces:** Produce `isNewer(candidate: string, current: string): boolean`, `fetchLatestRelease(fetcher?: typeof fetch): Promise<LatestRelease>`, and typed `UpdateError` with a localized message key and optional HTTP status.

- [x] Add tests for version ordering, missing releases, HTTP and network errors, timeout, malformed metadata, and unsafe release links.
- [x] Run `pnpm test -- src/lib/updates.test.ts` and observe missing-module failure.
- [x] Implement semantic version comparison, a 15-second request timeout, and validation of release metadata and repository release URLs.
- [x] Run the helper tests and confirm success.

### Task 2: Help integration

**Files:** Create `src/components/UpdateCheck.tsx`, `src/components/UpdateCheck.test.tsx`, `src/i18n/messages/updates.ts`; modify `src/components/HelpDialog.tsx`, `src/i18n/messages/index.ts`, `src/App.css`, `src-tauri/tauri.conf.json`, and `README.md`.

**Interfaces:** Consume Task 1 exports. Produce `UpdateCheck({ version, versionError }: { version: string | null; versionError: boolean })`.

- [x] Test explicit checks only, checking/available/current states, retry, version-load failure, and download destination.
- [x] Run the component tests before implementing the component.
- [x] Implement the component, all supported translations, Help integration, and the GitHub API CSP permission.
- [x] Run `pnpm typecheck`, `pnpm lint`, `pnpm format:check`, `pnpm test`, and `pnpm build` and inspect results.
- [x] Review the final diff for scope, error handling, production configuration, and translated copy.

## Execution Record

Proceed in the current isolated worktree under the user's implementation approval. No additional worktree, dependencies, or Rust source changes are needed. Leave changes available for user review without publishing.

- Helper and component tests each failed first because the implementation module did not exist, then passed after implementation.
- Final review: fresh reviewer inspected the full working diff, new files, approved spec, and review focus.
- Final: fixed mismatched release tag and download page — `rejects a download page for a different tag` failed before the fix and passed after it; final suite 222/222.
- Final: minor (deferred): an installed development build newer than the published release also displays “You're running the latest version.” This matches CloneFleet's existing wording; a future copy adjustment could say “No newer release is available.”
- Final verification: TypeScript, ESLint, Prettier, all 22 test files (222 tests), production Vite build, and `git diff --check` passed. The full test suite required execution outside the sandbox because existing release fixtures use Git's shell to push to temporary local repositories.
- Native packaged-webview network behavior was not exercised. No Rust source changed. The build reported a nonfatal chunk-size warning.
