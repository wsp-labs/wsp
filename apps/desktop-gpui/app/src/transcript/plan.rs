// SPDX-License-Identifier: AGPL-3.0-only
//! What the plan card draws of a plan.

/// proposedPlanTitle in the web app's lib/proposedPlan.ts: the plan's first heading.
pub(super) fn plan_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(heading).filter(|h| !h.is_empty())
}

/// A markdown heading's words, `^\s{0,3}#{1,6}\s+(.+)$`; Some("") for a heading with none.
fn heading(line: &str) -> Option<String> {
    let indent = line.len() - line.trim_start().len();
    let rest = &line[indent..];
    let hashes = rest.len() - rest.trim_start_matches('#').len();
    let text = &rest[hashes..];
    (indent <= 3 && (1..=6).contains(&hashes) && text.starts_with(char::is_whitespace)).then(|| text.trim().to_owned())
}

/// A plan the card shows collapsed until opened: over 900 characters or 20 lines.
pub fn plan_folds(markdown: &str) -> bool {
    markdown.encode_utf16().count() > 900 || markdown.split('\n').count() > 20
}

/// What the plan card draws: the plan less its title and a leading Summary heading, or while a long plan is
/// collapsed its first ten lines and "..." (stripDisplayedPlanMarkdown and buildCollapsedProposedPlanPreviewMarkdown).
pub fn plan_shown(markdown: &str, open: bool) -> String {
    let lines: Vec<&str> = markdown.trim_end().lines().collect();
    let mut source: Vec<&str> = match lines.first() {
        Some(first) if heading(first).is_some() => lines[1..].to_vec(),
        _ => lines.clone(),
    };
    let drop_blank = |source: &mut Vec<&str>| {
        while source.first().is_some_and(|l| l.trim().is_empty()) {
            source.remove(0);
        }
    };
    drop_blank(&mut source);
    if source.first().and_then(|l| heading(l)).is_some_and(|h| h.to_lowercase() == "summary") {
        source.remove(0);
        drop_blank(&mut source);
    }
    if open || !plan_folds(markdown) {
        return source.join("\n");
    }
    let mut preview: Vec<&str> = Vec::new();
    let (mut seen, mut more) = (0, false);
    for line in source.iter().map(|l| l.trim_end()) {
        let visible = !line.trim().is_empty();
        if visible && seen >= 10 {
            more = true;
            break;
        }
        preview.push(line);
        seen += usize::from(visible);
    }
    while preview.last().is_some_and(|l| l.trim().is_empty()) {
        preview.pop();
    }
    if preview.is_empty() {
        return plan_title(markdown).unwrap_or_else(|| "Plan preview unavailable.".to_owned());
    }
    if more {
        preview.extend(["", "..."]);
    }
    preview.join("\n")
}
