# wsp archive

This holds code cut from wsp before the Rust port, kept so it can be found again. Every piece lives at the path it had in wsp. The commit column is the git tag `archive/pre-rust-cut` in wsp-labs/wsp, the last commit before the cut.

| Piece | What it did | Where it lived | Commit | Why it was cut | Why it might come back |
|---|---|---|---|---|---|
| Snapshots | Took a project image of a running workspace's disk | `host/src/verbs/workspaces.ts`, `runtime/src/projects/workspaces.ts`, `protocol/src/words/image.ts`, op `workspaces.snapshot` | `archive/pre-rust-cut` | Every cloud child clones under the new model | Cloning in every child proves slow |
| Project images | One record per snapshot, `wsp image remove`, the listing | `protocol/src/views/snapshot-lineage.ts`, `runtime/src/images/records.ts`, ops `projectGoldens.list` and `.remove` | `archive/pre-rust-cut` | Goes with snapshots | Cost control for snapshots if they return |
| `new --from` | A create forked a project image | `protocol/src/wire/runtime.ts`, `runtime/src/types/wiring.ts` | `archive/pre-rust-cut` | Goes with project images | Returns with project images |
| Project image at add | Cloned, seeded and installed on a worker, then snapshotted | `runtime/src/project-landing.ts`, `runtime/src/projects/projects.ts` | `archive/pre-rust-cut` | The ruling clones every time | A per-provider project cache |
| Version pins | Recorded each tool's version and checksum at a seal | `protocol/src/views/golden-image.ts`, `catalog/src/roads.ts`, `engine/src/golden-tools.ts` | `archive/pre-rust-cut` | Images float to current versions | Copies drift and need to be reproducible |
| Spawn switch | Per computer and workspace on or off for threads starting other threads | `protocol/src/views/workspace.ts`, `protocol/src/place-state.ts`, `runtime/src/account/` | `archive/pre-rust-cut` | Caps on the root thread replace it | A kill switch against runaway spawning |
| Engine flags | Gave a workspace Docker or podman through a fenced socket | `protocol/src/views/golden-image.ts`, `runtime/src/images/records.ts`, host verbs | `archive/pre-rust-cut` | No engine in copies under the new model | Docker compose projects in cloud copies |
| init-upgrade | Applied only a recipe's diff to a kept builder | `host/src/init-upgrade.ts`, `engine/src/golden-diff.ts` | `archive/pre-rust-cut` | Kept builders go | Full rebuilds get slow |
| The `wsp init` wizard | Six screens to pick harnesses and tools, sign in, build and seal an image, and its job road (10 `init.*` ops) | `host/src/init.ts`, `host/src/cli/init.ts`, 19 `host/src/init-*.ts`, `protocol/src/init-job.ts` | `archive/pre-rust-cut` | The onboarding job replaces it | Its sign-in handoff and row logic are reused |
| `wsp recipe` and `recipe scan` | Read the Mac and wrote ticked rows with costs | `host/src/recipe-command.ts`, `recipe-answer.ts`, `recipe-custom.ts`, `collect/src/recipe.ts` | `archive/pre-rust-cut` | Named recipes replace recipe.json | The Mac read does the same reading |
| Recipe shapes and image planning | Types for recipe.json rows; planned image builds and copies | `protocol/src/views/golden-image.ts`, `host/src/image-recipe.ts` | `archive/pre-rust-cut` | Go with the recipe code | Return with it |
| Image export | Wrote the passphrase-sealed image file | `host/src/image.ts` | `archive/pre-rust-cut` | Images no longer move by hand | Moving an image by hand again |
| Live manifest | Printed this machine's manifest (`pnpm live`) | `collect/src/live.ts` | `archive/pre-rust-cut` | Dev-only | A dev check for the Mac read |

## What is in each row

### Snapshots

- `daemon/crates/wsp-mcp/record/tools/snapshot.json`, whole file
- `daemon/crates/wsp-mcp/tests/answers/snapshot.json`, whole file
- `packages/protocol/src/wire/runtime.ts.cut.diff`, cut diff
- `packages/protocol/src/views/snapshot-lineage.ts.cut.diff`, cut diff
- `packages/protocol/src/words/image.ts.cut.diff`, cut diff
- `packages/runtime/src/projects/workspaces.ts.cut.diff`, cut diff
- `packages/runtime/src/serve.ts.cut.diff`, cut diff
- `packages/host/src/storage.ts.cut.diff`, cut diff
- `packages/host/src/verbs/workspaces.ts.cut.diff`, cut diff
- `packages/host/src/verbs.ts.cut.diff`, cut diff
- `daemon/crates/wsp-mcp/src/tools/machine.rs.cut.diff`, cut diff
- `packages/host/test/storage.test.ts.cut.diff`, cut diff
- `packages/host/test/verbs-fork-image.test.ts.cut.diff`, cut diff
- `packages/host/test/mcp.test.ts.cut.diff`, cut diff
- `skills/wsp/SKILL.md.cut.diff`, cut diff
- `apps/web/src/protocol/client.ts.cut.diff`, cut diff

### Project images

- `daemon/crates/wsp-mcp/record/tools/image_remove.json`, whole file
- `daemon/crates/wsp-mcp/tests/answers/image_remove.json`, whole file
- `packages/runtime/test/project-golden.test.ts`, whole file
- `packages/protocol/src/wire/runtime.ts.cut.diff`, cut diff
- `packages/protocol/src/views/snapshot-lineage.ts.cut.diff`, cut diff
- `packages/protocol/src/views/golden-image.ts.cut.diff`, cut diff
- `packages/runtime/src/images/records.ts.cut.diff`, cut diff
- `packages/runtime/src/images/image.ts.cut.diff`, cut diff
- `packages/runtime/src/serve.ts.cut.diff`, cut diff
- `daemon/crates/wsp-mcp/src/tools/image.rs.cut.diff`, cut diff
- `skills/wsp/SKILL.md.cut.diff`, cut diff

### `new --from`

- `packages/protocol/src/wire/runtime.ts.cut.diff`, cut diff
- `packages/runtime/src/types/wiring.ts.cut.diff`, cut diff

### Project image at add

- `packages/runtime/src/projects/projects.ts.cut.diff`, cut diff
- `packages/runtime/src/project-landing.ts.cut.diff`, cut diff
- `packages/runtime/test/project-landing.test.ts.cut.diff`, cut diff

### Version pins

- `packages/protocol/src/views/golden-image.ts.cut.diff`, cut diff
- `packages/protocol/src/words/image.ts.cut.diff`, cut diff
- `packages/runtime/src/images/golden.ts.cut.diff`, cut diff
- `packages/engine/src/golden-tools.ts.cut.diff`, cut diff
- `packages/engine/src/golden.ts.cut.diff`, cut diff
- `packages/engine/src/image-vault.ts.cut.diff`, cut diff
- `daemon/crates/wsp-mcp/src/tools/image.rs.cut.diff`, cut diff
- `packages/runtime/test/sealed-image.test.ts.cut.diff`, cut diff
- `packages/engine/test/golden.test.ts.cut.diff`, cut diff
- `packages/engine/test/golden-import.test.ts.cut.diff`, cut diff
- `packages/protocol/test/format.test.ts.cut.diff`, cut diff

### Spawn switch

- `packages/protocol/src/views/workspace.ts.cut.diff`, cut diff
- `packages/protocol/src/views/place.ts.cut.diff`, cut diff
- `packages/protocol/src/workspace-state.ts.cut.diff`, cut diff
- `packages/protocol/src/place-state.ts.cut.diff`, cut diff
- `packages/protocol/src/words/computer.ts.cut.diff`, cut diff
- `packages/runtime/src/projects/workspaces.ts.cut.diff`, cut diff
- `packages/runtime/src/account/rules.ts.cut.diff`, cut diff
- `packages/runtime/src/machines/sub-spawn.ts.cut.diff`, cut diff
- `packages/host/src/verbs/workspaces.ts.cut.diff`, cut diff
- `packages/host/src/verbs/workspaces-help.ts.cut.diff`, cut diff
- `packages/host/src/verbs/computers.ts.cut.diff`, cut diff
- `packages/runtime/test/spawn-guard.test.ts.cut.diff`, cut diff
- `packages/runtime/test/places-settings.test.ts.cut.diff`, cut diff
- `skills/wsp/SKILL.md.cut.diff`, cut diff

### Engine flags

- `packages/protocol/src/wire/runtime.ts.cut.diff`, cut diff
- `packages/protocol/src/views/golden-image.ts.cut.diff`, cut diff
- `packages/protocol/src/views/place.ts.cut.diff`, cut diff
- `packages/runtime/src/images/records.ts.cut.diff`, cut diff
- `packages/runtime/src/types/wiring.ts.cut.diff`, cut diff
- `packages/host/test/verbs-fork-image.test.ts.cut.diff`, cut diff

### init-upgrade

- `packages/host/src/init-upgrade.ts`, whole file
- `packages/host/test/golden-update.live.test.ts`, whole file
- `packages/host/test/init-upgrade.test.ts`, whole file
- `packages/engine/src/golden-diff.ts.cut.diff`, cut diff
- `packages/engine/test/golden-diff.test.ts.cut.diff`, cut diff

### The `wsp init` wizard

- `apps/web/src/settings/ImageBuild.tsx`, whole file
- `apps/web/src/settings/ImageRecipe.tsx`, whole file
- `apps/web/src/settings/recipe/AgentLine.tsx`, whole file
- `apps/web/src/settings/recipe/BuildRows.tsx`, whole file
- `apps/web/src/settings/recipe/ReadingRows.tsx`, whole file
- `apps/web/src/settings/recipe/RecipeScreen.tsx`, whole file
- `apps/web/src/settings/recipe/RecipeStep.tsx`, whole file
- `apps/web/src/settings/recipe/RoadChoice.tsx`, whole file
- `apps/web/src/settings/recipe/SignInMark.tsx`, whole file
- `apps/web/src/settings/recipe/ansi.ts`, whole file
- `apps/web/src/settings/recipe/useRecipeJob.ts`, whole file
- `apps/web/test/edit-image-doors.test.tsx`, whole file
- `apps/web/test/fixtures/keyRefusedJob.ts`, whole file
- `apps/web/test/image-build.test.tsx`, whole file
- `apps/web/test/image-recipe-layout.browser.test.ts`, whole file
- `apps/web/test/image-recipe.test.tsx`, whole file
- `apps/web/test/image-rows.test.tsx`, whole file
- `apps/web/test/recipe/index.html`, whole file
- `apps/web/test/recipe/main.tsx`, whole file
- `apps/web/test/sign-in-code.test.tsx`, whole file
- `apps/web/test/wireframe-agents.test.ts`, whole file
- `apps/web/test/wireframe/agents.ts`, whole file
- `packages/host/src/cli/init.ts`, whole file
- `packages/host/src/init-also.ts`, whole file
- `packages/host/src/init-beside.ts`, whole file
- `packages/host/src/init-earlier.ts`, whole file
- `packages/host/src/init-first.ts`, whole file
- `packages/host/src/init-host-signins.ts`, whole file
- `packages/host/src/init-job.ts`, whole file
- `packages/host/src/init-local.ts`, whole file
- `packages/host/src/init-log.ts`, whole file
- `packages/host/src/init-opening.ts`, whole file
- `packages/host/src/init-pick.ts`, whole file
- `packages/host/src/init-screens.ts`, whole file
- `packages/host/src/init-serve.ts`, whole file
- `packages/host/src/init-summary.ts`, whole file
- `packages/host/src/init-table.ts`, whole file
- `packages/host/src/init-times.ts`, whole file
- `packages/host/src/init-vault.ts`, whole file
- `packages/host/src/init-weight.ts`, whole file
- `packages/host/src/init.ts`, whole file
- `packages/host/test/golden-import.live.test.ts`, whole file
- `packages/host/test/init-also.test.ts`, whole file
- `packages/host/test/init-beside.test.ts`, whole file
- `packages/host/test/init-first.test.ts`, whole file
- `packages/host/test/init-job.test.ts`, whole file
- `packages/host/test/init-local.test.ts`, whole file
- `packages/host/test/init-log.test.ts`, whole file
- `packages/host/test/init-opening.test.ts`, whole file
- `packages/host/test/init-pick.test.ts`, whole file
- `packages/host/test/init-screens.test.ts`, whole file
- `packages/host/test/init-select.ascii.test.ts`, whole file
- `packages/host/test/init-select.test.ts`, whole file
- `packages/host/test/init-serve.test.ts`, whole file
- `packages/host/test/init-signin.test.ts`, whole file
- `packages/host/test/init-stream.test.ts`, whole file
- `packages/host/test/init-table.test.ts`, whole file
- `packages/host/test/init-times.test.ts`, whole file
- `packages/host/test/init-vault.test.ts`, whole file
- `packages/host/test/init-weight.test.ts`, whole file
- `packages/host/test/init.test.ts`, whole file
- `packages/host/test/signin.live.test.ts`, whole file
- `packages/protocol/src/init-job.ts`, whole file
- `packages/protocol/test/init-job.test.ts`, whole file
- `packages/protocol/src/wire/runtime.ts.cut.diff`, cut diff
- `packages/protocol/src/wire/capabilities.ts.cut.diff`, cut diff
- `packages/protocol/src/wire/events-union.ts.cut.diff`, cut diff
- `packages/protocol/src/words/init.ts.cut.diff`, cut diff
- `packages/protocol/src/words/units.ts.cut.diff`, cut diff
- `packages/runtime/src/types/wiring.ts.cut.diff`, cut diff
- `packages/runtime/src/serve.ts.cut.diff`, cut diff
- `packages/engine/src/golden-diff.ts.cut.diff`, cut diff
- `packages/engine/src/tool-sizes.ts.cut.diff`, cut diff
- `packages/host/src/init-handoff.ts.cut.diff`, cut diff
- `packages/host/src/init-select.ts.cut.diff`, cut diff
- `packages/host/src/init-recipe.ts.cut.diff`, cut diff
- `packages/host/src/init-signin.ts.cut.diff`, cut diff
- `packages/host/src/init-import.ts.cut.diff`, cut diff
- `packages/host/src/init-layout.ts.cut.diff`, cut diff
- `packages/host/src/cli/commands.ts.cut.diff`, cut diff
- `packages/host/src/cli/serve.ts.cut.diff`, cut diff
- `packages/host/src/cli/help.ts.cut.diff`, cut diff
- `packages/runtime/test/serve.test.ts.cut.diff`, cut diff
- `packages/engine/test/tool-sizes.test.ts.cut.diff`, cut diff
- `packages/protocol/test/format.test.ts.cut.diff`, cut diff
- `packages/host/test/host.test.ts.cut.diff`, cut diff
- `packages/host/test/init-import.test.ts.cut.diff`, cut diff
- `packages/host/test/init-recipe.test.ts.cut.diff`, cut diff
- `packages/host/test/init-handoff.test.ts.cut.diff`, cut diff
- `packages/host/test/init-layout.test.ts.cut.diff`, cut diff
- `packages/host/test/place-provision.test.ts.cut.diff`, cut diff
- `packages/host/test/keys.test.ts.cut.diff`, cut diff
- `apps/web/test/add-cloud.test.tsx.cut.diff`, cut diff
- `apps/web/test/computers.test.tsx.cut.diff`, cut diff
- `apps/web/test/notices-host.test.tsx.cut.diff`, cut diff
- `apps/web/test/needs-you.test.tsx.cut.diff`, cut diff
- `skills/wsp/SKILL.md.cut.diff`, cut diff
- `apps/web/src/settings/add/AddCloudDialog.tsx.cut.diff`, cut diff
- `apps/web/src/settings/add/cloudFlow.ts.cut.diff`, cut diff
- `apps/web/src/settings/ImageCard.tsx.cut.diff`, cut diff
- `apps/web/src/protocol/client.ts.cut.diff`, cut diff
- `apps/web/src/protocol/store/useStore.ts.cut.diff`, cut diff
- `apps/web/src/notices/hostNotices.ts.cut.diff`, cut diff

### `wsp recipe` and `recipe scan`

- `daemon/crates/wsp-mcp/record/tools/recipe.json`, whole file
- `daemon/crates/wsp-mcp/record/tools/recipe_scan.json`, whole file
- `daemon/crates/wsp-mcp/src/tools/recipe.rs`, whole file
- `daemon/crates/wsp-mcp/tests/answers/recipe.json`, whole file
- `daemon/crates/wsp-mcp/tests/answers/recipe_scan.json`, whole file
- `packages/collect/src/recipe.ts`, whole file
- `packages/collect/test/recipe.test.ts`, whole file
- `packages/host/src/recipe-answer.ts`, whole file
- `packages/host/src/recipe-command.ts`, whole file
- `packages/host/src/recipe-custom.ts`, whole file
- `packages/host/src/scan.ts`, whole file
- `packages/host/test/recipe-command.test.ts`, whole file
- `packages/host/test/recipe-custom.test.ts`, whole file
- `packages/host/test/recipe-scan.test.ts`, whole file
- `packages/host/test/scan.test.ts`, whole file
- `packages/host/src/verbs/projects.ts.cut.diff`, cut diff
- `packages/host/src/verbs.ts.cut.diff`, cut diff
- `daemon/crates/wsp-mcp/src/tools/said.rs.cut.diff`, cut diff
- `packages/host/test/mcp.test.ts.cut.diff`, cut diff
- `packages/host/test/skill.test.ts.cut.diff`, cut diff
- `skills/wsp/SKILL.md.cut.diff`, cut diff

### Recipe shapes and image planning

- `packages/host/src/image-recipe.ts`, whole file
- `packages/host/test/golden-recipe.test.ts`, whole file
- `packages/host/test/image-recipe.test.ts`, whole file
- `packages/protocol/src/views/golden-image.ts.cut.diff`, cut diff

### Image export

- `packages/host/src/image.ts`, whole file
- `packages/protocol/src/views/golden-image.ts.cut.diff`, cut diff
- `packages/protocol/src/words/image.ts.cut.diff`, cut diff
- `packages/runtime/src/types/wiring.ts.cut.diff`, cut diff
- `packages/runtime/src/serve.ts.cut.diff`, cut diff
- `packages/engine/src/image-vault.ts.cut.diff`, cut diff
- `packages/engine/src/vault-cipher.ts.cut.diff`, cut diff
- `packages/host/src/verbs/workspaces.ts.cut.diff`, cut diff
- `packages/host/src/verbs.ts.cut.diff`, cut diff
- `packages/runtime/test/serve.test.ts.cut.diff`, cut diff
- `packages/engine/test/vault-cipher.test.ts.cut.diff`, cut diff
- `packages/host/test/verbs-fork-image.test.ts.cut.diff`, cut diff
- `skills/wsp/SKILL.md.cut.diff`, cut diff

### Live manifest

- `packages/collect/src/live.ts`, whole file

## How to read a cut diff

A cut diff is the `git diff` from the last commit before the cut to the cut commit, for a file that stays in wsp. Lines starting with `-` are the archived code. Lines starting with `+` are small edits made so what stayed still works. A cut diff also mixes in parts of the cut that were deleted outright rather than archived. Those parts are listed in the next section.

## Cut and not kept here

These were deleted outright. They are only in git history at the tag `archive/pre-rust-cut` in wsp-labs/wsp.

- The cap loop
- The secrets profile (`host/src/init-secrets.ts`)
- The value-writing half of serverValues
- The recipe.json readers and writers (`host/src/recipe-file.ts`)
- The dead words of `new --from`

## Still in wsp

These stay in wsp and are not archived here. The image machinery: `engine/src/golden*.ts` and `runtime/src/images/`.

The ops `init.get` and `init.keys` also stay in wsp until the owner decides.
