// SPDX-License-Identifier: AGPL-3.0-only
// The reply, view and event shapes a client reads off the wire, by their exported names, beside the protocol number
// they are held to. `protocol-shapes.json` records each one's fields, and the ops, as they were when the number last
// moved, and `test/protocol-shapes.test.ts` fails a shape that lost, renamed or retyped a field, or an op gone or
// renamed, while the number stood: the change an older client misreads. The test also requires every shape the GPUI
// app's own record (`apps/desktop-gpui/host-client/record/protocol.json`) reads to be listed or inside a listed one, so
// a shape the app starts reading moves that record and the test asks for its name here.

/** Every name is a zod schema `packages/protocol/src/index.ts` exports. The Rust host's wsp-wire mirrors this list. */
export const WIRE_SHAPE_NAMES: readonly string[] = [
  "AccountView", "AccountsAnswer", "ActivityEnded", "ActivityView", "AgentRow", "AgentSetupView", "AgentsReport",
  "AgentsSignInEvent", "AgentsSignInRun", "AppProcList", "AppProcsEvent", "AuthReply", "BringBackResult",
  "BrokerStatus", "Capabilities", "CheckoutReply", "CommitDraft", "ConversationRead", "ConversationsAnswer",
  "DaemonChannelEvent", "DaemonOpenReply", "DeviceView", "EditorChoice", "EditorId", "EventUnion",
  "EventsSubscribeReply", "FixResult", "GitCommitReply", "GitDiscardReply", "GitPrDiffReply", "GitPrReactReply",
  "GitPrReplyReply", "GitPrResolveReply", "GitUpdateReply", "GoldenManifest", "HarnessCatalog", "HistoryPage",
  "HostFolderListing", "InitSetup", "JoinMint", "KeptAttachment", "MergeResult", "PendingComputer", "PlaceAddJob",
  "PlaceDial", "PlaceDoorView", "PlaceEstimate", "PlaceHolds", "PlaceJoinDevice", "PlaceRemoved", "PlaceSetup",
  "PlaceSysEvent", "PlaceUpdateReply", "PlaceView", "PluginRow", "PortForward", "PortProbeView", "PortReachView",
  "Preferences", "ProjectAddStage", "ProjectBranch", "ProjectExportResult", "ProjectExportStage",
  "ProjectImportResult", "ProjectImportStage", "ProjectView", "ProviderKeyWorks", "PullRequest", "ReadingsAnswer",
  "RecipeView", "ReleaseView", "ResetAnswer", "ReviewDraft", "ReviewPostResult", "SealedImageBuilt",
  "SealedImageView", "ServerToolsAnswer", "SessionAccessResult", "SessionAnswerResult", "SessionAsideResult",
  "SessionEvent", "SessionInterruptResult", "SessionRenameResult", "SessionRestoreResult", "SessionRewindResult",
  "SessionRunEvent", "SessionSearchResult", "SessionSettleResult", "SessionStartResult", "SessionSteerResult",
  "SessionView", "SessionWarmResult", "SettledThread", "SkillAdded", "SkillHit", "SkillPreview", "SlateEventAnswer",
  "SlateReadAnswer", "SlateStateAnswer", "SlateView", "SlateWriteAnswer", "SlatesGetAnswer", "SlatesImageAnswer",
  "SlatesPageAnswer", "SlatesResolveAnswer", "SnapshotLineage", "SnapshotRollbackResult", "SnapshotStorage",
  "SshHostSuggestion", "StartResult", "SubStarted", "TerminalConfig", "ThreadDefaults", "ThreadHead", "ThreadView",
  "UsedAnswer", "ViewedMarks", "WorkspaceCostEvent", "WorkspaceCreateResult", "WorkspaceLanding", "WorkspaceOut",
  "WorkspaceStatus", "WorkspaceSysEvent", "WorkspaceView",
];
