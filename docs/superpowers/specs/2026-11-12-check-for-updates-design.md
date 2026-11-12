# Check for updates in Help

## Intent

Let users discover newer Marduk releases using the manual update-check flow in CloneFleet. The user selected placement in the existing Help dialog.

## Approaches

- **Help dialog (selected):** put the check beside the installed version, reusing the existing dialog and external-link handling.
- **Separate About dialog:** mirrors CloneFleet's placement but adds another dialog. The user chose Help instead.

## Behavior

Add a “Check for updates” button beside the installed version in Help. Only clicking the button contacts GitHub's latest-release endpoint for `keeganwitt/marduk-agent-sessions`.

Show a checking state and disable the button while the request runs. Compare the published release version with the installed bundle version. Show either an up-to-date message or the available version and a Download link to its GitHub release page, opened in the system browser.

Show localized, retryable errors for unavailable releases, network failures, unsuccessful HTTP responses, and malformed release data. If the installed version cannot be read, report that failure rather than claim the app is current. Localize the new interface text using the existing catalogs.

## Constraints and scope

Use the existing React, TypeScript, Tauri version API, and external-link handler. No background checks, persisted preferences, installer downloads, or automatic installation. Permit the GitHub API request in the production content security policy.

## Implementation decisions

The builder will choose helper structure, timeout handling, version-validation details, and styling consistent with the Help dialog. Handle numerical version ordering correctly and validate release links before offering them.

## Validation

Test version comparisons, successful and failed release requests, update-available and up-to-date UI states, disabled checks during loading, and recovery after errors. Run the frontend checks and production build, plus applicable repository checks for any Rust changes.
