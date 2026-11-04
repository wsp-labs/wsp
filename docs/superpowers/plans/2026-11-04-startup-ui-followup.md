# Connected-child startup follow-up

The live Hetzner check successfully copied and ran the child, but exposed three presentation failures: per-chunk copy progress became separate setup rows, pending creation ancestry was discarded, and the selected setup page became a fresh composer after the workspace landed. A real Shell regression also reproduced first-turn events being dropped during the initial empty history read.

The runtime now marks progress updates as updates of one step and carries the destination computer. Pending creation retains the authenticated asking thread and joins that tree using the existing sidebar components. Connected setup and Boat share StartupLine; the completed connected startup is projected through the existing SubRow view without setting the provider lifecycle record. Its file step says that the current project was copied, and it has no provider delete countdown. Failed restarts stamp the shared failure state.

Handoff follows the incoming child thread. Live events arriving before its first history reply are retained, and an empty history reply cannot discard their thread. The regression renders Shell, opens setup once, then receives the first reply and another streamed delta without a second click.

Validation: 341 focused web/runtime tests passed; the shared startup, existing Boat startup, store/reload, sidebar and actual Shell transcript paths are included. The existing 43 tree tests and the Experimental shell/settings tests were also checked. Native protocol regeneration, strict TypeScript checks and Rust MCP checks pass. Electron preview 0.3.9+boatfix.2506.6 is built separately from the owner's running preview, ad hoc signed, and passes eight packaged-app smoke cases. No release or production restart.

Area-note proposals: a pending child's workspace.creating.askedBy joins its opener before the first session exists; an initial empty history reply must retain live events received while it was in flight. No AGENTS.md is edited on this branch.

# Desktop feature audit

Bounded source/history audit: v0.3.8 through shipped v0.3.9, then later pushed main UI commits through f669c283e0821ccda9bbc09554f91286d28ae157. GPUI rendering/performance changes are excluded. No confirmed GPUI-only product addition was found in this range. The shipped GPUI squash 4372ebfb65 (PR 1710) ports the existing screens. Later slate facts/frame/look work, 7f0d89ef61, changes both apps and runtime on main; the pinned hotfix excludes it in both clients. This is not a complete screen-by-screen parity claim.

The Experimental switch is an incomplete handoff, not an absent Electron control. apps/web/src/settings/general.tsx renders it only when reads.experimental is not null. settingsReads.ts asks the desktop bridge only on the local host. apps/desktop/src/experimental.ts requires <home>/desktop/desktop-gpui.path to name an existing absolute executable. However apps/desktop-gpui/app/src/main.rs calls desktop::switch::retire on normal launch, which deletes that path and the experimental flag, and no native discovery writer or return action was found. The Electron implementation and tests remain; native discovery/return is missing under the restored Electron-default policy. The earlier statement that there already was a working native return path was incorrect. This audit makes no toggle changes.
