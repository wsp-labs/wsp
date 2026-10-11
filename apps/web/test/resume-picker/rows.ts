// SPDX-License-Identifier: AGPL-3.0-only
// Rows shaped like the owner's wsp folder on 2026-10-11 (48 rows there: 42 Claude Code, 6 Codex; one transcript
// 132.7 MB), cut to a screenful, for the #2054 design round. Titles, branches, sizes and ages are made up to look like his.
import type { ConversationLine, OutsideConversation } from "@wsp/protocol";

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const ago = (ms: number): number => Date.now() - ms;
const MB = 1024 * 1024;
const KB = 1024;

export interface PickRow extends OutsideConversation {
  readonly project: string;
}

const row = (r: Omit<PickRow, "origin" | "live"> & { live?: boolean }): PickRow => ({ origin: "terminal", live: false, ...r });

export const WSP_ROWS: PickRow[] = [
  row({ project: "pr_wsp", agent: "claude", id: "8f1c2a90-1", title: "WSP context and workflow", firstPrompt: "Read AGENTS.md and the map, then tell me how you'd run today's tickets", branch: "experimental", cwd: "/Users/zingzy/wsp", lastAt: ago(6 * MIN), bytes: 2.4 * MB, live: true }),
  row({ project: "pr_wsp", agent: "claude", id: "8f1c2a90-2", title: "Slate launch video scripts", firstPrompt: "Write the three launch video scripts for the slate", branch: "experimental", cwd: "/Users/zingzy/wsp", lastAt: ago(42 * MIN), bytes: 18.7 * MB }),
  row({ project: "pr_wsp", agent: "codex", id: "codex-1735-r2", title: "Review 1735 round 2 (Codex)", firstPrompt: "Review PR 1301 round 2 against the ticket", branch: "fix/1735-signins-held", cwd: "/Users/zingzy/wsp-work/t1735", lastAt: ago(70 * MIN), bytes: 640 * KB }),
  row({ project: "pr_wsp", agent: "claude", id: "8f1c2a90-4", title: "Sign-ins held on a new box", firstPrompt: "The box keeps saying Signed in while the token is held", branch: "fix/1735-signins-held", cwd: "/Users/zingzy/wsp-work/t1735", lastAt: ago(3 * HOUR), bytes: 46.1 * MB }),
  row({ project: "pr_wsp", agent: "claude", id: "8f1c2a90-5", title: "Resume picker plan", firstPrompt: "Plan the resume picker, palette and project menu", branch: "experimental", cwd: "/Users/zingzy/wsp", lastAt: ago(5 * HOUR), bytes: 1.1 * MB, thread: "s5" }),
  row({ project: "pr_wsp", agent: "codex", id: "codex-docs-search", title: "Docs search is slow on the reference pages", firstPrompt: "Docs search is slow on the reference pages", branch: "main", cwd: "/Users/zingzy/wsp", lastAt: ago(9 * HOUR), bytes: 212 * KB }),
  row({ project: "pr_wsp", agent: "claude", id: "897699a0", title: "WSP website rework", firstPrompt: "This session is being continued from a previous conversation that ran out of context", branch: "experimental", cwd: "/Users/zingzy/wsp", lastAt: ago(26 * HOUR), bytes: 41.2 * MB }),
  row({ project: "pr_wsp", agent: "claude", id: "73df5281", title: "WSP website rework", firstPrompt: "Rework the website: new hero, feature grid, pricing", branch: "experimental", cwd: "/Users/zingzy/wsp", lastAt: ago(2 * DAY + 3 * HOUR), bytes: 132.7 * MB }),
  row({ project: "pr_wsp", agent: "claude", id: "8f1c2a90-9", title: "Usage page: one ink per account", firstPrompt: "On the usage page every account should get its own ink", branch: "ticket/2011-usage-inks", cwd: "/Users/zingzy/wsp-work/t2011", lastAt: ago(2 * DAY + 9 * HOUR), bytes: 7.9 * MB }),
  row({ project: "pr_wsp", agent: "codex", id: "codex-musl", title: "Bump the daemon's musl target", firstPrompt: "Bump the daemon's musl target and rebuild the release", branch: "main", cwd: "/Users/zingzy/wsp", lastAt: ago(4 * DAY), bytes: 96 * KB }),
  row({ project: "pr_wsp", agent: "claude", id: "8f1c2a90-11", title: "Plan the launch checklist", firstPrompt: "Plan the launch checklist from the map", branch: "experimental", cwd: "/Users/zingzy/wsp", lastAt: ago(6 * DAY), bytes: 22 * MB }),
  row({ project: "pr_wsp", agent: "claude", id: "8f1c2a90-12", title: "Why does the relay drop the socket after 60 s?", firstPrompt: "Why does the relay drop the socket after 60 s?", branch: "main", cwd: "/Users/zingzy/wsp", lastAt: ago(9 * DAY), bytes: 4.8 * MB }),
  row({ project: "pr_wsp", agent: "claude", id: "8f1c2a90-13", title: "Check the box's disk before the image build", firstPrompt: "Check the box's disk before the image build", branch: "main", cwd: "/Users/zingzy/wsp", lastAt: ago(11 * DAY), bytes: 18 * KB }),
];

export const OTHER_ROWS: PickRow[] = [
  row({ project: "pr_spoo", agent: "claude", id: "spoo-1", title: "Short links redirect twice", firstPrompt: "Why do the short links 302 twice before they land?", branch: "fix/double-redirect", cwd: "/Users/zingzy/spoo", lastAt: ago(3 * HOUR + 20 * MIN), bytes: 1.3 * MB }),
  row({ project: "pr_spoo", agent: "codex", id: "spoo-2", title: "Cart total rounding", firstPrompt: "round the cart total once, at the end", branch: "main", cwd: "/Users/zingzy/spoo", lastAt: ago(DAY + 2 * HOUR), bytes: 404 * KB }),
  row({ project: "pr_spoo", agent: "claude", id: "spoo-3", title: "QR code on every short link page", firstPrompt: "Add a QR code to every short link page", branch: "feat/qr-codes", cwd: "/Users/zingzy/spoo", lastAt: ago(3 * DAY), bytes: 3.1 * MB }),
  row({ project: "pr_landing", agent: "claude", id: "landing-1", title: "Landing hero copy, round 3", firstPrompt: "Third pass on the hero copy", branch: "main", cwd: "/root/spoo-landing", lastAt: ago(2 * DAY), bytes: 860 * KB }),
];

/** The newest messages a preview shows, oldest first, and how many came before them. */
export interface Preview {
  readonly lines: ConversationLine[];
  readonly earlier: number;
}

const P = (who: ConversationLine["who"], text: string, tool?: string): ConversationLine => ({ who, text, ...(tool !== undefined ? { tool } : {}) });

export const PREVIEWS: Record<string, Preview> = {
  "8f1c2a90-1": {
    earlier: 61,
    lines: [
      P("person", "Read AGENTS.md and the map, then tell me how you'd run today's tickets."),
      P("agent", "The map has six open tickets after yesterday's round. I'd start the two research passes first, since #2054 and #2049 touch the same files, then batch the four small fixes into one builder."),
      P("tool", "{\"command\":\"gh issue view 2049 -R wsp-labs/wsp-map\"}", "Bash"),
      P("person", "Do #2049 first, it blocks the picker."),
      P("agent", "Starting #2049. The daemon refuses more than 64 folders in one frame, so the list will split the request and merge the answers. I'll post the build report on the ticket."),
    ],
  },
  "8f1c2a90-2": {
    earlier: 38,
    lines: [
      P("person", "Write the three launch video scripts for the slate: watching a deploy, a checklist, the usage chart. 40 seconds each."),
      P("agent", "Here are the three. Each opens on the panel already beside the chat, so the first frame shows the slate doing work, not a title card."),
      P("tool", "{\"file_path\":\"docs/launch/slate-scripts.md\"}", "Write"),
      P("person", "Cut the second one to 30 s and lose the line about cron."),
      P("agent", "Done. Script two runs 30 s now: the checklist ticks itself off as the build steps land, and the cron line is gone."),
    ],
  },
  "codex-1735-r2": {
    earlier: 9,
    lines: [
      P("person", "Review PR 1301 round 2 against the ticket. Same checklist as round 1."),
      P("tool", "{\"command\":\"gh pr diff 1301\"}", "Bash"),
      P("agent", "Round 2 fixes all three blockers. One should-fix is left: the sign-in row still reads Signed in for the second or so the token is held, because the row reads the old answer until the next poll."),
    ],
  },
  "897699a0": {
    earlier: 18,
    lines: [
      P("person", "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion."),
      P("agent", "Picking up at the pricing section. The hero and the feature grid are done and pushed."),
      P("person", "Ship the pricing section with the three tiers and stop there."),
      P("agent", "Pricing is in: three tiers, monthly and yearly, the yearly toggle on the right. I stopped there as asked."),
    ],
  },
  "73df5281": {
    earlier: 412,
    lines: [
      P("person", "The feature grid needs real screenshots, not the grey boxes."),
      P("tool", "{\"file_path\":\"apps/site/src/components/FeatureGrid.tsx\"}", "Edit"),
      P("agent", "Swapped in the six screenshots from the release folder. The grid is done; pricing is next."),
      P("person", "Keep going with pricing."),
      P("agent", "I'm close to the end of this conversation's room, so I'll write a summary you can continue from."),
    ],
  },
};

export const previewOf = (row: PickRow): Preview =>
  PREVIEWS[row.id] ?? {
    earlier: 4,
    lines: [P("person", row.firstPrompt ?? row.title), P("agent", "Done. The change is in and the tests pass; the branch is ready for review.")],
  };
