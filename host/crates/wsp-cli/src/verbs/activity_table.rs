// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/activity-table.ts` at 65f6b89c7: `wsp activity` as Settings > Activity reads, the
//! totals, then wsp's own, each thread heaviest first and No thread last, a turn's run script and input pump folded
//! into its agent's row, the figures in aligned columns and nothing wider than the terminal. `--wide` adds the pid
//! and the whole command line and folds nothing. Spec: `spec/activity.md`.

use wsp_wire::views::activity::ActivityView;

/// `NAME_MIN` (activity-table.ts:13): the narrowest a name column is cut to before the kind column goes instead.
pub const NAME_MIN: usize = 16;
/// `LEFT_MAX` (activity-table.ts:15): the widest the names and kinds stand, whatever the terminal.
pub const LEFT_MAX: usize = 72;

/// The table's two choices: `width` in cells, `None` where the line has no terminal to redraw on (the TS
/// `Infinity`), and `wide`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableOpts {
    pub width: Option<usize>,
    pub wide: bool,
}

/// `activityTable` (activity-table.ts:91-134).
pub fn activity_table(view: &ActivityView, o: TableOpts) -> String {
    let _ = (view, o);
    todo!("activity-table.ts:91-134")
}
