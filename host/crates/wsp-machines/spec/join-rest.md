# The second join: wsp-core's and wsp-machines' leftover slices

What the join settled between the two crates, and what is still open.

## Settled
- `ImportLedger` and the engine's `Builder` (golden.ts:522, :405) live in `wsp_core::types::pending` with the TS's fields,
  `ImportLedger` in the TS's key order (`recipeHash, recipe, applied, smoke, missingTools, silenced, shellNoise,
  leftBehind`) and `applied` as the `ImportStage` enum. `images::engine` re-exports them (`Builder` as `EngineBuilder`).
- One builder type: `wsp_core::types::internal::LiveBuilder`, held as `images::BuilderRef` (`Rc<RefCell<..>>`). The boot's
  `HeldBuilder` is gone; `BootRuntime::builders` answers `images::Builders`.
- `HarnessAdapterFactory` answers `Result`; wsp-core's own `AdapterFactory` is gone.
- Harness catalog rows write their keys in the TS's order: wsp-wire's `HarnessCatalog` fields follow the table row, then
  the adapter's declarations, then the person's shaping. Its shape fixtures mark the three cases `anyOrder`.

## Still open
- `setupPatched`, `agentEnvRefusal`, `configDirRefusal`, `noConfigDirLine` and `notRunRefusal` stay in
  `wsp-core/src/protocol_pending.rs`: none has landed in wsp-words yet.
- `images::engine::GoldenRoads` stays, with no Rust port behind it: the engine has not ported `golden.ts` or
  `image-vault.ts`. It carries `buildGolden`, `prepareBuilder`, `sealGolden`, `upgradeBuilder`, `applyGoldenImport`,
  `applyDelta`, `killUntilGone`, `readGone`, `promoteVersion`, `rollback`, `refreshPreviewToken`, `importImageVault`,
  `refuseForeignMembers`, `imageHash`. Its shapes (`ImageVault`, `SealResult`, `KillConfirm`, `GoldenDelta`, `Templates`)
  stay in `images/engine.rs` until the engine ports them.
- Not joined: wsp-machines' `LandingRoad` (machines/create.rs) still takes `deps: &serde_json::Value` instead of wsp-core's
  `ProjectLanding` and `LandingDeps`; `agents_read`'s port handles are still its own empty traits beside wsp-core's.
