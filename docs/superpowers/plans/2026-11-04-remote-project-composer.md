# Browse and register projects on connected computers

Keep repository cloning and local-folder seeding unchanged. Add explicit existing-folder registration for a connected computer. Inspect its path and Git metadata on that computer, record the folder in place, and never clone, install or delete its contents. Use the existing host.folders/on transport in both desktop dialogs, with per-computer read state and stale-answer protection. Keep the existing project and permission rules.

- [x] Prove remote browsing and existing-folder registration fail on the current source.
- [x] Add optional existing=true on projects.add; expose --existing in CLI, the MCP input, and skill in one contract. Validate the computer and path, and read metadata remotely. Preserve local-folder seed behavior.
- [x] Wire Classic and native composers to selected-computer folder listings, path browsing, duplicate indicators, existing-folder Add, and honest errors/loading. Providers retain repository clone mode.
- [x] Test remote-only paths, plain folders, duplicate/invalid targets, no writes to the remote folder, local behavior and switching computers during a read. Existing remote-folder tests retain offline/version refusal coverage.
- [x] Regenerate contracts/docs, run focused checks and laws, cold review, update PR 1766 and build a local preview.

Proposed runtime note: Existing-folder registration reads the path and Git metadata on the named computer and records that exact folder; seeding a local folder remains a separate explicit request.
Proposed web note: Folder reads are keyed to the selected computer and late answers cannot populate a different computer's picker.

Verification so far: 93 focused project/browser tests, 132 UI/CLI/skill parity tests, 555 law tests (22 existing skips), Rust MCP suite, TypeScript checks, real Git folder inspection, and native cargo check --tests passed. Native UI has not been driven live for this change.

Isolated-copy reassessment: joined-computer runtimes, checkout copying, folder packaging, selected-file transfer, child retention and cleanup already exist. The remaining work is integration: stage the lead thread's exact current checkout on the destination, create the isolated runtime there, attach its child/tree lifetime, and return changes even without a shared Git remote. Existing-folder registration does not create that cross-computer copy relationship.

Local Electron preview: 0.3.9+boatfix.2506.4. Eight packaged-app smoke cases passed on isolated throwaway hosts. Cold review found and fixed reselecting the current computer clearing its list; that regression was proven red before the guard. The PR retains the stable v0.3.9 base, so pre-review ancestry checks against current main remain inapplicable; the laws and focused checks run directly.

Additional proposed note lines: successful cloud removal refreshes cached setup keys; an accepted non-queued retry restores broker access even without a previous result or retention timer.
