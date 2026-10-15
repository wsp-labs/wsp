// SPDX-License-Identifier: AGPL-3.0-only
// Every kind of thread status, in the order a thread is read against them:
// a question outranks a running turn, a turn held for a slot and a machine
// being made are read before one that runs, a usage limit is read before a plain
// failure, a failure outranks a deleted machine, which outranks a disk past its keep, which outranks a stop, a stop outranks work not pushed, which outranks a finish nobody has seen, and resting takes
// whatever is left, so it stays last. A kind is its module and its line here.
import { DONE } from "./done.js";
import { EXPIRED } from "./expired.js";
import { FAILED } from "./failed.js";
import { LIMITED } from "./limited.js";
import type { StatusKind } from "./kind.js";
import { NEEDS_YOU } from "./needs-you.js";
import { NOT_PUSHED } from "./not-pushed.js";
import { RELEASED } from "./released.js";
import { RESTING } from "./resting.js";
import { RESUMING } from "./resuming.js";
import { STARTING } from "./starting.js";
import { STOPPED } from "./stopped.js";
import { WAITING } from "./waiting.js";
import { WORKING } from "./working.js";

export type { StatusKind, StatusTone, ThreadStatusInput } from "./kind.js";

export const THREAD_STATUS_KINDS: readonly StatusKind[] = [NEEDS_YOU, WAITING, STARTING, WORKING, LIMITED, RESUMING, FAILED, RELEASED, EXPIRED, STOPPED, NOT_PUSHED, DONE, RESTING];
