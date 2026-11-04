//! macOS: attach mpv's own (borderless) window underneath our transparent player window.
//!
//! mpv's `wid` option is not honoured by its macOS video output, so instead we let mpv create its
//! window, make it a child window ordered *below* our window and keep its frame glued to our
//! content area. Our window stays transparent, so the HTML overlay (controls, OSD, sidebar)
//! is drawn over mpv's native Metal/Vulkan video. All functions must run on the main thread.
use objc2::{rc::Retained, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSWindow, NSWindowOrderingMode};
use std::collections::HashSet;

fn windows(mtm: MainThreadMarker) -> Vec<Retained<NSWindow>> {
    NSApplication::sharedApplication(mtm).windows().iter().collect()
}

/// Addresses of all current app windows (taken before mpv starts).
pub fn snapshot(mtm: MainThreadMarker) -> HashSet<usize> {
    windows(mtm).iter().map(|w| Retained::as_ptr(w) as usize).collect()
}

/// Find the window mpv created (any window not present in `known`), tuck it under `parent` and
/// size it to the parent's content area. Returns its address, or None if it does not exist yet.
pub fn adopt(mtm: MainThreadMarker, parent_ptr: usize, known: &HashSet<usize>) -> Option<usize> {
    let parent = windows(mtm).into_iter().find(|w| Retained::as_ptr(w) as usize == parent_ptr)?;
    let child = windows(mtm).into_iter().find(|w| {
        let p = Retained::as_ptr(w) as usize;
        p != parent_ptr && !known.contains(&p)
    })?;
    child.setIgnoresMouseEvents(true);
    child.setHasShadow(false);
    unsafe { parent.addChildWindow_ordered(&child, NSWindowOrderingMode::Below) };
    sync(&parent, &child);
    Some(Retained::as_ptr(&child) as usize)
}

fn sync(parent: &NSWindow, child: &NSWindow) {
    let rect = parent.contentRectForFrameRect(parent.frame());
    child.setFrame_display(rect, true);
}

/// Re-fit the child window to the parent (call on resize / fullscreen changes).
pub fn resync(mtm: MainThreadMarker, parent_ptr: usize, child_ptr: usize) {
    let all = windows(mtm);
    let parent = all.iter().find(|w| Retained::as_ptr(w) as usize == parent_ptr);
    let child = all.iter().find(|w| Retained::as_ptr(w) as usize == child_ptr);
    if let (Some(p), Some(c)) = (parent, child) {
        sync(p, c);
    }
}
