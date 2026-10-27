// SPDX-License-Identifier: AGPL-3.0-only
// The reply shapes a client reads off the wire, beside the protocol number they are held to. `protocol-shapes.json`
// records each shape's fields as they were when the number last moved, and `test/protocol-shapes.test.ts` fails a
// shape that lost, renamed or retyped a field while the number stood: the change an older client misreads.
import type { ZodTypeAny } from "zod";
import { AgentsReport } from "../agents-report.js";
import { PullRequest } from "../pull-request.js";
import { RecipeView } from "../recipe-file.js";
import { ReleaseView } from "../release.js";
import { SlateReadAnswer, SlateView, SlateWriteAnswer } from "../slate/wire.js";
import { AgentSetupView } from "../thread-defaults.js";
import { AccountsAnswer, UsedAnswer } from "../usage.js";
import { ActivityView } from "../views/activity.js";
import { PlaceRemoved, PlaceUpdateReply, PlaceView } from "../views/place.js";
import { HostFolderListing, ProjectExportResult, ProjectImportResult } from "../views/project-bundle.js";
import { SessionView, ThreadView } from "../views/session.js";
import { SealedImageView, SnapshotRollbackResult, WorkspaceCreateResult } from "../views/snapshot-lineage.js";
import { ProjectView, WorkspaceView } from "../views/workspace.js";
import { Capabilities } from "./capabilities.js";
import { DeviceView } from "./devices.js";
import { EventsSubscribeReply } from "./events-union.js";
import { AuthReply } from "./protocol-number.js";
import {
  BrokerStatus, SessionAsideResult, SessionRestoreResult, SessionRewindResult, SessionSearchResult, SessionSettleResult, SessionStartResult,
  SessionWarmResult, SettledThread,
} from "./runtime.js";

/** Every reply shape the app, the command line and the tool server read, by name. A shape a client reads is added here
 * in the change that adds it; a name here is a name in `protocol-shapes.json`. */
export const WIRE_REPLY_SHAPES: Readonly<Record<string, ZodTypeAny>> = {
  AuthReply,
  Capabilities,
  EventsSubscribeReply,
  ReleaseView,
  BrokerStatus,
  PlaceView,
  PlaceUpdateReply,
  PlaceRemoved,
  DeviceView,
  WorkspaceView,
  WorkspaceCreateResult,
  ProjectView,
  HostFolderListing,
  ProjectImportResult,
  ProjectExportResult,
  SessionView,
  ThreadView,
  SessionStartResult,
  SessionSettleResult,
  SessionRestoreResult,
  SessionSearchResult,
  SessionAsideResult,
  SessionRewindResult,
  SessionWarmResult,
  SettledThread,
  ActivityView,
  AccountsAnswer,
  UsedAnswer,
  SealedImageView,
  SnapshotRollbackResult,
  RecipeView,
  AgentSetupView,
  AgentsReport,
  PullRequest,
  SlateView,
  SlateReadAnswer,
  SlateWriteAnswer,
};
