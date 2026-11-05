// SPDX-License-Identifier: AGPL-3.0-only
import { z } from "zod";

/** The opening carried before child setup starts, shared by provider and connected-computer starts.
 * These are the requested picks; the agent's accepted picks arrive with session.start. */
export const SubStartIntent = z.object({
  prompt: z.string(),
  harness: z.string().optional(),
  model: z.string().optional(),
  effort: z.string().optional(),
  contextWindow: z.string().optional(),
  permissionMode: z.string().optional(),
  access: z.string().optional(),
  fast: z.boolean().optional(),
});
export type SubStartIntent = z.infer<typeof SubStartIntent>;
