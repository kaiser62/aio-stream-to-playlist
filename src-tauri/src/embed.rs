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

/// A rectangle in the parent's content area, in CSS pixels from the top-left.
pub type Rect = Option<(f64, f64, f64, f64)>;

/// Find the window mpv created (any window not present in `known`), tuck it under `parent` and
/// place it on `rect`. Returns its address, or None if it does not exist yet.
pub fn adopt(mtm: MainThreadMarker, parent_ptr: usize, known: &HashSet<usize>, rect: Rect) -> Option<usize> {
    let parent = windows(mtm).into_iter().find(|w| Retained::as_ptr(w) as usize == parent_ptr)?;
    let child = windows(mtm).into_iter().find(|w| {
        let p = Retained::as_ptr(w) as usize;
        p != parent_ptr && !known.contains(&p)
    })?;
    child.setIgnoresMouseEvents(true);
    child.setHasShadow(false);
    unsafe { parent.addChildWindow_ordered(&child, NSWindowOrderingMode::Below) };
    sync(&parent, &child, rect);
    Some(Retained::as_ptr(&child) as usize)
}

fn sync(parent: &NSWindow, child: &NSWindow, rect: Rect) {
    match rect {
        Some((x, y, w, h)) if w >= 2.0 && h >= 2.0 => {
            // The webview sits in the window's layout rect (below any title bar), not the full frame.
            let frame = parent.frame();
            let lay = parent.contentLayoutRect();
            let mut r = frame;
            r.origin.x += lay.origin.x;
            r.origin.y += lay.origin.y;
            r.size = lay.size;
            let ch = r.size.height;
            r.origin.x += x;
            r.origin.y += ch - y - h;
            r.size.width = w;
            r.size.height = h;
            child.setAlphaValue(1.0);
            child.setFrame_display(r, true);
            // Native fullscreen reshuffles child ordering and can leave mpv's window above the webview
            // (controls/OSD vanish): re-assert "below the parent" and the same window level every sync.
            child.setLevel(parent.level());
            child.orderWindow_relativeTo(NSWindowOrderingMode::Below, parent.windowNumber());
        }
        _ => child.setAlphaValue(0.0), // no visible video area: hide it
    }
}

/// Re-fit the child window (call on resize / move / fullscreen changes and when the UI moves the video area).
pub fn resync(mtm: MainThreadMarker, parent_ptr: usize, child_ptr: usize, rect: Rect) {
    let all = windows(mtm);
    let parent = all.iter().find(|w| Retained::as_ptr(w) as usize == parent_ptr);
    let child = all.iter().find(|w| Retained::as_ptr(w) as usize == child_ptr);
    if let (Some(p), Some(c)) = (parent, child) {
        sync(p, c, rect);
    }
}
