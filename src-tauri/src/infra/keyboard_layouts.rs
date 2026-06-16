//! Keyboard-layout provider: builds, per installed layout, a map from each
//! layout character to the Latin character on the SAME physical key. The
//! frontend tries these maps in both directions so a Latin project name typed
//! on a non-Latin layout (or vice-versa) still matches.
//!
//! On macOS the maps are derived from the OS via Text Input Sources
//! (`TISCreateInputSourceList` + `UCKeyTranslate`) — not hardcoded — so every
//! script the user actually has installed (Russian, Armenian, …) is covered
//! exactly. Other platforms get an empty list and search degrades to raw +
//! fuzzy with no behavior change.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;

/// One installed keyboard layout: `map[layout_char] = latin_char_on_same_key`.
#[derive(Debug, Clone, Serialize)]
pub struct LayoutMap {
    pub id: String,
    pub map: BTreeMap<char, char>,
}

/// Strategy for obtaining the user's keyboard-layout maps. One implementation
/// per OS, selected once at the composition root.
pub trait KeyboardLayoutProvider: Send + Sync {
    fn layouts(&self) -> Vec<LayoutMap>;
}

/// The provider for the OS this binary was compiled for.
pub fn platform_layout_provider() -> Arc<dyn KeyboardLayoutProvider> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::MacLayoutProvider)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(NoopLayoutProvider)
    }
}

/// Fallback for platforms without a layout provider: no maps.
#[cfg(not(target_os = "macos"))]
struct NoopLayoutProvider;

#[cfg(not(target_os = "macos"))]
impl KeyboardLayoutProvider for NoopLayoutProvider {
    fn layouts(&self) -> Vec<LayoutMap> {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
pub use macos::watch_layout_changes;

#[cfg(target_os = "macos")]
mod macos {
    use super::{KeyboardLayoutProvider, LayoutMap};
    use core_foundation::base::TCFType;
    use core_foundation::data::CFData;
    use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
    use core_foundation_sys::base::{CFRelease, CFTypeRef};
    use core_foundation_sys::data::CFDataRef;
    use core_foundation_sys::string::CFStringRef;
    use std::collections::BTreeMap;
    use std::os::raw::c_void;
    use std::ptr;
    use tauri::{AppHandle, Emitter};

    type TisInputSourceRef = CFTypeRef;
    type Boolean = u8;
    type CFNotificationCenterRef = *const c_void;

    // Physical key → US-QWERTY Latin letter (virtual key codes are fixed
    // regardless of the active layout). We translate each of these keys
    // through a layout to learn which character it produces there.
    const LETTER_KEYS: &[(u16, char)] = &[
        (0x00, 'a'), (0x0B, 'b'), (0x08, 'c'), (0x02, 'd'), (0x0E, 'e'),
        (0x03, 'f'), (0x05, 'g'), (0x04, 'h'), (0x22, 'i'), (0x26, 'j'),
        (0x28, 'k'), (0x25, 'l'), (0x2E, 'm'), (0x2D, 'n'), (0x1F, 'o'),
        (0x23, 'p'), (0x0C, 'q'), (0x0F, 'r'), (0x01, 's'), (0x11, 't'),
        (0x20, 'u'), (0x09, 'v'), (0x0D, 'w'), (0x07, 'x'), (0x10, 'y'),
        (0x06, 'z'),
    ];

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        static kTISPropertyUnicodeKeyLayoutData: CFStringRef;
        static kTISNotifyEnabledKeyboardInputSourcesChanged: CFStringRef;
        fn TISCreateInputSourceList(properties: CFTypeRef, include_all: Boolean) -> CFArrayRef;
        fn TISGetInputSourceProperty(source: TisInputSourceRef, key: CFStringRef) -> CFTypeRef;
        fn LMGetKbdType() -> u8;
        #[allow(clippy::too_many_arguments)]
        fn UCKeyTranslate(
            key_layout_ptr: *const u8,
            virtual_key_code: u16,
            key_action: u16,
            modifier_key_state: u32,
            keyboard_type: u32,
            key_translate_options: u32,
            dead_key_state: *mut u32,
            max_string_length: u32,
            actual_string_length: *mut u32,
            unicode_string: *mut u16,
        ) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFNotificationCenterGetDistributedCenter() -> CFNotificationCenterRef;
        fn CFNotificationCenterAddObserver(
            center: CFNotificationCenterRef,
            observer: *const c_void,
            callback: extern "C" fn(
                CFNotificationCenterRef,
                *mut c_void,
                CFStringRef,
                *const c_void,
                *const c_void,
            ),
            name: CFStringRef,
            object: *const c_void,
            suspension_behavior: isize,
        );
    }

    pub struct MacLayoutProvider;

    impl KeyboardLayoutProvider for MacLayoutProvider {
        fn layouts(&self) -> Vec<LayoutMap> {
            current_layouts()
        }
    }

    /// Build the `layout_char → latin_char` map for one layout, given a
    /// translator for that layout's keys. Pure (no FFI) so it is unit-testable;
    /// identity entries (Latin layouts) are dropped.
    fn build_map(translate: impl Fn(u16) -> Option<char>) -> BTreeMap<char, char> {
        let mut map = BTreeMap::new();
        for &(keycode, latin) in LETTER_KEYS {
            if let Some(ch) = translate(keycode) {
                let lc = ch.to_lowercase().next().unwrap_or(ch);
                if lc != latin {
                    map.insert(lc, latin);
                }
            }
        }
        map
    }

    /// Translate a single physical key through a layout's `UCKeyboardLayout`
    /// data, with no modifiers and dead keys disabled.
    unsafe fn translate_key(layout: *const u8, keycode: u16, kbd_type: u32) -> Option<char> {
        let mut dead: u32 = 0;
        let mut buf = [0u16; 4];
        let mut len: u32 = 0;
        let status = UCKeyTranslate(
            layout,
            keycode,
            0,            // kUCKeyActionDown
            0,            // no modifiers
            kbd_type,     // LMGetKbdType()
            1,            // kUCKeyTranslateNoDeadKeysMask
            &mut dead,
            buf.len() as u32,
            &mut len,
            buf.as_mut_ptr(),
        );
        if status != 0 || len == 0 {
            return None;
        }
        String::from_utf16_lossy(&buf[..len as usize]).chars().next()
    }

    /// Enumerate the currently-enabled keyboard layouts and build one map each.
    /// Input sources without layout data (input methods) are skipped.
    pub fn current_layouts() -> Vec<LayoutMap> {
        unsafe {
            let list = TISCreateInputSourceList(ptr::null(), 0);
            if list.is_null() {
                return Vec::new();
            }
            let count = CFArrayGetCount(list);
            let kbd_type = LMGetKbdType() as u32;
            let mut out = Vec::new();
            for i in 0..count {
                let src = CFArrayGetValueAtIndex(list, i) as TisInputSourceRef;
                if src.is_null() {
                    continue;
                }
                let data_ref =
                    TISGetInputSourceProperty(src, kTISPropertyUnicodeKeyLayoutData) as CFDataRef;
                if data_ref.is_null() {
                    continue; // no UnicodeKeyLayoutData → e.g. an input method
                }
                let data = CFData::wrap_under_get_rule(data_ref);
                let bytes = data.bytes();
                if bytes.is_empty() {
                    continue;
                }
                let layout_ptr = bytes.as_ptr();
                let map = build_map(|kc| translate_key(layout_ptr, kc, kbd_type));
                if !map.is_empty() {
                    out.push(LayoutMap {
                        id: format!("src{i}"),
                        map,
                    });
                }
            }
            CFRelease(list as CFTypeRef);
            out
        }
    }

    /// Observe the system notification that fires when enabled keyboard layouts
    /// change (added/removed/changed) and re-emit the fresh maps to the
    /// frontend. Registered on the main thread so the callback runs there too.
    pub fn watch_layout_changes(app: AppHandle) {
        unsafe {
            let center = CFNotificationCenterGetDistributedCenter();
            if center.is_null() {
                return;
            }
            // Leak the handle for the app's lifetime; the callback borrows it.
            let observer = Box::into_raw(Box::new(app)) as *const c_void;
            CFNotificationCenterAddObserver(
                center,
                observer,
                on_layouts_changed,
                kTISNotifyEnabledKeyboardInputSourcesChanged,
                ptr::null(),
                4, // CFNotificationSuspensionBehaviorDeliverImmediately
            );
        }
    }

    extern "C" fn on_layouts_changed(
        _center: CFNotificationCenterRef,
        observer: *mut c_void,
        _name: CFStringRef,
        _object: *const c_void,
        _user_info: *const c_void,
    ) {
        if observer.is_null() {
            return;
        }
        let app = unsafe { &*(observer as *const AppHandle) };
        let _ = app.emit("layouts-changed", current_layouts());
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn build_map_composes_a_positional_layout() {
            // Simulate ЙЦУКЕН: pressing physical g/r/o/w/p/a/y yields п/к/щ/ц/з/ф/н.
            let sim = |kc: u16| match kc {
                0x05 => Some('п'),
                0x0F => Some('к'),
                0x1F => Some('щ'),
                0x0D => Some('ц'),
                0x23 => Some('з'),
                0x00 => Some('ф'),
                0x10 => Some('н'),
                _ => None,
            };
            let m = build_map(sim);
            assert_eq!(m.get(&'п'), Some(&'g'));
            assert_eq!(m.get(&'к'), Some(&'r'));
            assert_eq!(m.get(&'н'), Some(&'y'));
        }

        #[test]
        fn build_map_drops_identity_latin_layout() {
            // A US layout: every key yields its own Latin letter → all identity.
            let sim = |kc: u16| {
                LETTER_KEYS
                    .iter()
                    .find(|(k, _)| *k == kc)
                    .map(|(_, c)| *c)
            };
            assert!(build_map(sim).is_empty());
        }

        #[test]
        fn build_map_uppercases_are_lowercased() {
            let sim = |kc: u16| if kc == 0x05 { Some('П') } else { None };
            let m = build_map(sim);
            assert_eq!(m.get(&'п'), Some(&'g'));
        }
    }
}
