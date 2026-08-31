//! Keyboard-layout provider: builds, per installed layout, a map from each
//! layout character to the Latin character on the SAME physical key. The
//! frontend tries these maps in both directions so a Latin project name typed
//! on a non-Latin layout (or vice-versa) still matches.
//!
//! The maps are derived from the OS, never hardcoded, so every script the user
//! actually has installed (Russian, Armenian, …) is covered exactly: macOS
//! reads Text Input Sources (`TISCreateInputSourceList` + `UCKeyTranslate`),
//! X11 reads the keymap through GDK. Anywhere else the list is empty and
//! search degrades to raw + fuzzy with no behavior change.

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
        Arc::new(linux::X11LayoutProvider)
    }
}

#[cfg(target_os = "macos")]
pub use macos::watch_layout_changes;

#[cfg(not(target_os = "macos"))]
pub use linux::watch_layout_changes;

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

#[cfg(not(target_os = "macos"))]
mod linux {
    use super::{KeyboardLayoutProvider, LayoutMap};
    use std::collections::BTreeMap;
    use std::sync::RwLock;
    use tauri::{AppHandle, Emitter};

    // Physical key → US-QWERTY Latin letter on that same key. X11 hardware
    // keycodes are the evdev codes plus 8 and never move, whatever layout is
    // active — the exact counterpart of the macOS virtual key codes above.
    const LETTER_KEYS: &[(u32, char)] = &[
        (38, 'a'), (56, 'b'), (54, 'c'), (40, 'd'), (26, 'e'), (41, 'f'),
        (42, 'g'), (43, 'h'), (31, 'i'), (44, 'j'), (45, 'k'), (46, 'l'),
        (58, 'm'), (57, 'n'), (32, 'o'), (33, 'p'), (24, 'q'), (27, 'r'),
        (39, 's'), (28, 't'), (30, 'u'), (55, 'v'), (25, 'w'), (53, 'x'),
        (29, 'y'), (52, 'z'),
    ];

    /// The maps last read from the X keymap. GDK may only be touched from the
    /// main thread while `layouts()` answers a command on a worker thread, so
    /// the maps are built on the main thread — at startup and whenever the
    /// keymap changes — and merely served from here.
    static CACHE: RwLock<Vec<LayoutMap>> = RwLock::new(Vec::new());

    pub struct X11LayoutProvider;

    impl KeyboardLayoutProvider for X11LayoutProvider {
        fn layouts(&self) -> Vec<LayoutMap> {
            CACHE.read().map(|maps| maps.clone()).unwrap_or_default()
        }
    }

    /// Build the `layout_char → latin_char` map for one layout group, given a
    /// translator for that group's keys. Pure (no GDK) so it is unit-testable;
    /// identity entries (Latin layouts) are dropped.
    fn build_map(translate: impl Fn(u32) -> Option<char>) -> BTreeMap<char, char> {
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

    /// Read every layout group out of the X keymap and build one map each.
    /// MAIN THREAD ONLY — GDK is not thread-safe.
    pub fn current_layouts() -> Vec<LayoutMap> {
        use gtk::gdk;

        let Some(keymap) = gdk::Display::default().and_then(|d| gdk::Keymap::for_display(&d)) else {
            return Vec::new();
        };

        // One `entries_for_keycode` call reports every (group, level) the key
        // carries, so the number of installed groups falls out of the data —
        // GDK exposes no count of its own. Level 0 is the unshifted character.
        let mut chars: BTreeMap<(i32, u32), char> = BTreeMap::new();
        let mut groups = 0;
        for &(keycode, _) in LETTER_KEYS {
            for (key, keyval) in keymap.entries_for_keycode(keycode) {
                groups = groups.max(key.group() + 1);
                if key.level() != 0 {
                    continue;
                }
                if let Some(ch) = gdk::keys::Key::from(keyval).to_unicode() {
                    chars.insert((key.group(), keycode), ch);
                }
            }
        }

        (0..groups)
            .filter_map(|group| {
                let map = build_map(|keycode| chars.get(&(group, keycode)).copied());
                (!map.is_empty()).then(|| LayoutMap {
                    id: format!("group{group}"),
                    map,
                })
            })
            .collect()
    }

    /// Seed the cache and keep it fresh: GDK raises `keys-changed` whenever the
    /// X keymap is rebuilt (a layout added, removed or reordered). Registered
    /// on the main thread, so the callback runs there too.
    pub fn watch_layout_changes(app: AppHandle) {
        use gtk::gdk;

        refresh(&app);
        let Some(keymap) = gdk::Display::default().and_then(|d| gdk::Keymap::for_display(&d)) else {
            return;
        };
        keymap.connect_keys_changed(move |_| refresh(&app));
    }

    /// Re-read the keymap into the cache and push the fresh maps to the
    /// frontend. MAIN THREAD ONLY (see `current_layouts`).
    fn refresh(app: &AppHandle) {
        let maps = current_layouts();
        if let Ok(mut cache) = CACHE.write() {
            *cache = maps.clone();
        }
        let _ = app.emit("layouts-changed", maps);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn build_map_composes_a_cyrillic_layout() {
            // Physical c/o/n/n on a Russian layout yield ц/о/н/н, so a query
            // typed there converts straight back to "conn".
            let sim = |kc: u32| match kc {
                54 => Some('ц'),
                32 => Some('о'),
                57 => Some('н'),
                33 => Some('п'),
                38 => Some('а'),
                29 => Some('ы'),
                _ => None,
            };
            let m = build_map(sim);
            assert_eq!(m.get(&'ц'), Some(&'c'));
            assert_eq!(m.get(&'о'), Some(&'o'));
            assert_eq!(m.get(&'н'), Some(&'n'));
            assert_eq!(m.get(&'ы'), Some(&'y'));
        }

        #[test]
        fn build_map_drops_identity_latin_layout() {
            // A US layout: every key yields its own Latin letter → all identity.
            let sim = |kc: u32| LETTER_KEYS.iter().find(|(k, _)| *k == kc).map(|(_, c)| *c);
            assert!(build_map(sim).is_empty());
        }

        #[test]
        fn build_map_lowercases_uppercase_keysyms() {
            let sim = |kc: u32| if kc == 54 { Some('Ц') } else { None };
            assert_eq!(build_map(sim).get(&'ц'), Some(&'c'));
        }

        #[test]
        fn build_map_skips_keys_the_layout_does_not_produce() {
            let sim = |kc: u32| if kc == 54 { Some('ц') } else { None };
            assert_eq!(build_map(sim).len(), 1);
        }
    }
}
