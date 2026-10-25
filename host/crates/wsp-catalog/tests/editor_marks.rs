// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/editor-marks.test.ts.
use std::collections::HashSet;

use regex::Regex;
use wsp_catalog::marks::{editor_mark, editor_marks};
use wsp_catalog::pending::EditorId;

const EDITOR_IDS: [EditorId; 14] = [
    EditorId::Vscode,
    EditorId::Cursor,
    EditorId::VscodeInsiders,
    EditorId::Zed,
    EditorId::Idea,
    EditorId::Webstorm,
    EditorId::Pycharm,
    EditorId::Goland,
    EditorId::Rustrover,
    EditorId::Clion,
    EditorId::Phpstorm,
    EditorId::Rubymine,
    EditorId::Rider,
    EditorId::Finder,
];

mod editor_marks {
    use super::*;

    #[test]
    fn every_editor_the_host_opens_has_its_mark_finder_s_among_them_each_editor_under_o() {
        let missing: Vec<EditorId> = EDITOR_IDS.iter().copied().filter(|id| editor_mark(*id).is_none()).collect();
        assert_eq!(missing, []);
        let claimed: Vec<String> = editor_marks().iter().flat_map(|m| m.editors.iter().map(|e| format!("{e:?}"))).collect();
        let unique: HashSet<&String> = claimed.iter().collect();
        assert_eq!(unique.len(), claimed.len());
        assert_eq!(editor_mark(EditorId::Rustrover).map(|m| m.id.as_str()), Some("jetbrains"));
        assert!(editor_mark(EditorId::Cursor).unwrap().mark.svg.contains("M22.106 5.68"));
    }

    #[test]
    fn every_mark_is_t3_s_drawing_simple_icons_path_or_lucide_s_folder_at_a_pinned_rele() {
        let source = Regex::new(concat!(
            r"^https://github\.com/(pingdotgg/t3code/blob/d15210c/apps/web/src/components/Icons\.tsx",
            r"|simple-icons/simple-icons/blob/16\.30\.0/icons/[a-z]+\.svg",
            r"|lobehub/lobe-icons/blob/[0-9a-f]+/src/Cursor/components/Mono\.tsx",
            r"|lucide-icons/lucide/blob/0\.564\.0/icons/folder\.svg)$"
        ))
        .unwrap();
        assert!(!editor_marks().is_empty());
        for m in editor_marks() {
            assert!(source.is_match(&m.mark.source), "{}", m.id);
            assert!(["MIT", "CC0-1.0", "ISC"].contains(&m.mark.license.as_str()), "{}", m.id);
        }
    }
}
