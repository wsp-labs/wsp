// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of attachments.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::attachments::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_attachments.json");

/// 4 cases: what zod's parse of Attachment wrote, or the first issue it raised.
#[test]
fn attachment_reads_and_writes_as_zod_does() {
    cases::<Attachment>(FIXTURES, "Attachment");
}

/// 6 cases: what zod's parse of AttachmentRecord wrote, or the first issue it raised.
#[test]
fn attachment_record_reads_and_writes_as_zod_does() {
    cases::<AttachmentRecord>(FIXTURES, "AttachmentRecord");
}

/// 3 cases: what zod's parse of KeptAttachment wrote, or the first issue it raised.
#[test]
fn kept_attachment_reads_and_writes_as_zod_does() {
    cases::<KeptAttachment>(FIXTURES, "KeptAttachment");
}
