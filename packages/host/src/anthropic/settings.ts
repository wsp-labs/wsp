// SPDX-License-Identifier: AGPL-3.0-only

import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

/** Where the person's Claude Code keeps its config, and the environment and home it was read from. */
export interface ClaudeHere {
  env?: NodeJS.ProcessEnv;
  home?: string;
}

/** CLAUDE_CONFIG_DIR as the person wrote it, which also names their Keychain item, else ~/.claude. */
export const configDirOf = ({ env = process.env, home = homedir() }: ClaudeHere): string => env["CLAUDE_CONFIG_DIR"] || join(home, ".claude");

/** The person's user settings, read when asked; nothing where the file is missing or unreadable. */
export async function settingsOf(here: ClaudeHere): Promise<{ apiKeyHelper?: unknown; env?: Record<string, unknown> }> {
  try {
    const settings = JSON.parse(await readFile(join(configDirOf(here), "settings.json"), "utf8")) as unknown;
    return typeof settings === "object" && settings !== null ? settings : {};
  } catch {
    return {};
  }
}
