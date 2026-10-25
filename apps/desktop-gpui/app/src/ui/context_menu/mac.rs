// SPDX-License-Identifier: AGPL-3.0-only
//! The menu as AppKit pops it for Electron's `Menu.popup`: `-[NSMenu popUpMenuPositioningItem:atLocation:inView:]`.
//! Every item's target is one object of a class declared once, which keeps the picked item's tag and does nothing
//! else, so a pick never re-enters GPUI inside AppKit's tracking loop.

use std::sync::OnceLock;

use cocoa::base::{id, nil, BOOL, NO, YES};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};

/// One row as AppKit takes it: a key equivalent is the key's characters and its modifier mask.
pub struct Row {
    pub label: String,
    pub group: &'static str,
    pub enabled: bool,
    pub tooltip: Option<String>,
    pub key: Option<(String, u64)>,
}

/// `NSEventModifierFlags`, as `setKeyEquivalentModifierMask:` takes them.
pub const SHIFT: u64 = 1 << 17;
pub const CONTROL: u64 = 1 << 18;
pub const OPTION: u64 = 1 << 19;
pub const COMMAND: u64 = 1 << 20;

/// `NSUpArrowFunctionKey` and the three after it.
pub fn arrow(key: &str) -> Option<char> {
    let code = match key {
        "up" => 0xF700,
        "down" => 0xF701,
        "left" => 0xF702,
        "right" => 0xF703,
        _ => return None,
    };
    char::from_u32(code)
}

fn target_class() -> &'static Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    let class = *CLASS.get_or_init(|| {
        let mut decl = ClassDecl::new("WspContextMenuTarget", class!(NSObject)).expect("the class is declared once");
        decl.add_ivar::<isize>("pick");
        extern "C" fn pick(this: &mut Object, _: Sel, sender: id) {
            // SAFETY: AppKit sends the action with the chosen NSMenuItem, which answers `tag`.
            unsafe {
                let tag: isize = msg_send![sender, tag];
                this.set_ivar("pick", tag);
            }
        }
        // SAFETY: the method's signature is the action's, `- (void)pick:(id)sender`.
        unsafe { decl.add_method(sel!(pick:), pick as extern "C" fn(&mut Object, Sel, id)) };
        decl.register() as *const Class as usize
    });
    // SAFETY: a registered class lives as long as the process.
    unsafe { &*(class as *const Class) }
}

unsafe fn ns(text: &str) -> id {
    unsafe { NSString::alloc(nil).init_str(text).autorelease() }
}

/// The target a popped menu's items send to, which holds the pick until it is read and then let go.
pub struct Target(id);

impl Target {
    /// The picked row's index; none while nothing was picked.
    pub fn picked(&self) -> Option<usize> {
        // SAFETY: the target is retained until this value is dropped.
        let pick: isize = unsafe { *(*self.0).get_ivar("pick") };
        usize::try_from(pick).ok()
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        // SAFETY: `pop` made the target with `new`, so this drops the one reference it holds.
        unsafe {
            let _: () = msg_send![self.0, release];
        }
    }
}

/// Pops the rows at `at`, in points from the top left of `ns_view`, and tracks the menu until it closes: the target,
/// and whether AppKit says a row was chosen.
///
/// # Safety
/// `ns_view` is a live NSView in a window, and this runs on the main thread outside any GPUI update, since the
/// tracking loop runs the main queue GPUI dispatches its work to.
pub unsafe fn pop(ns_view: id, at: (f64, f64), rows: &[Row]) -> (Target, bool) {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let target: id = msg_send![target_class(), new];
        (*target).set_ivar::<isize>("pick", -1);
        let menu: id = msg_send![class!(NSMenu), alloc];
        let menu: id = msg_send![menu, initWithTitle: ns("")];
        let _: () = msg_send![menu, setAutoenablesItems: NO];
        for (ix, row) in rows.iter().enumerate() {
            if ix > 0 && rows[ix - 1].group != row.group {
                let rule: id = msg_send![class!(NSMenuItem), separatorItem];
                let _: () = msg_send![menu, addItem: rule];
            }
            let (key, mask) = row.key.clone().unwrap_or_default();
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![item, initWithTitle: ns(&row.label) action: sel!(pick:) keyEquivalent: ns(&key)];
            let _: () = msg_send![item, setKeyEquivalentModifierMask: mask];
            let _: () = msg_send![item, setTarget: target];
            let _: () = msg_send![item, setTag: ix as isize];
            let _: () = msg_send![item, setEnabled: if row.enabled { YES } else { NO }];
            if let Some(tip) = &row.tooltip {
                let _: () = msg_send![item, setToolTip: ns(tip)];
            }
            let _: () = msg_send![menu, addItem: item];
            let _: () = msg_send![item, release];
        }
        // GPUI's view is not flipped, so AppKit counts its points up from the bottom.
        let flipped: BOOL = msg_send![ns_view, isFlipped];
        let bounds: NSRect = msg_send![ns_view, bounds];
        let y = if flipped == NO { bounds.size.height - at.1 } else { at.1 };
        let chosen: BOOL = msg_send![menu, popUpMenuPositioningItem: nil atLocation: NSPoint::new(at.0, y) inView: ns_view];
        let _: () = msg_send![menu, release];
        pool.drain();
        (Target(target), chosen != NO)
    }
}
