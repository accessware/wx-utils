//! Whether the keyboard layout in use can press a chord at all, and which US key a key sits on.
//!
//! A chord names a key by the character it types without Shift. A punctuation character can live
//! anywhere on another layout, or only behind Shift, as `;` does on a Finnish keyboard; a chord on
//! it then matches no key event, and the list says so rather than showing a shortcut that does
//! nothing. An app can instead match such a chord by the key's place, with [`us_punctuation_at`].

use super::KeyChord;

/// The punctuation a US keyboard types without Shift, by the PC scan code of its key.
#[cfg(any(windows, all(unix, not(target_os = "macos")), test))]
const US_PUNCTUATION: [(u16, char); 11] = [
	(0x0C, '-'),
	(0x0D, '='),
	(0x1A, '['),
	(0x1B, ']'),
	(0x27, ';'),
	(0x28, '\''),
	(0x29, '`'),
	(0x2B, '\\'),
	(0x33, ','),
	(0x34, '.'),
	(0x35, '/'),
];

/// Linux numbers keys by their evdev code plus 8, and evdev codes are PC scan codes for these keys.
#[cfg(all(unix, not(target_os = "macos")))]
const X11_KEYCODE_OFFSET: u32 = 8;

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
fn us_punctuation_for_scan_code(scan_code: u32) -> Option<char> {
	US_PUNCTUATION.iter().find(|&&(scan, _)| u32::from(scan) == scan_code).map(|&(_, ch)| ch)
}

/// The punctuation the key behind a key event types on a US keyboard without Shift, or `None` for
/// any other key. `raw_key_flags` is the event's `get_raw_key_flags()`.
///
/// Match a chord on this when the key's own character matches nothing. On a Hungarian keyboard
/// the key where a US one has `;` types `é`, so a `;` shortcut can't be pressed by its character;
/// by its place it can. GTK reports `Shift+,` as `<`, so `Shift+,` needs it on any layout. Letters,
/// digits and named keys never come back, so a layout's own letters stay letters.
///
/// The flags hold the scan code on Windows and the hardware key code on Linux. macOS reports keys
/// by the characters the layout gives them and moves a menu's shortcuts to match, so there is
/// nothing to look up and this is always `None`.
#[must_use]
pub fn us_punctuation_at(raw_key_flags: u32) -> Option<char> {
	#[cfg(windows)]
	{
		// The extended bit marks the numeric keypad's `/` and Enter, which share their scan codes with the main keys.
		if raw_key_flags & (1 << 24) != 0 {
			return None;
		}
		us_punctuation_for_scan_code((raw_key_flags >> 16) & 0xFF)
	}
	#[cfg(all(unix, not(target_os = "macos")))]
	{
		us_punctuation_for_scan_code(raw_key_flags.checked_sub(X11_KEYCODE_OFFSET)?)
	}
	#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
	{
		let _ = raw_key_flags;
		None
	}
}

/// Whether `chord`'s key is one [`us_punctuation_at`] can return, so an app that matches by place
/// can always press it.
#[cfg(any(windows, test))]
fn is_us_punctuation(chord: &KeyChord) -> bool {
	let mut chars = chord.key.chars();
	matches!((chars.next(), chars.next()), (Some(ch), None) if US_PUNCTUATION.iter().any(|&(_, us)| us == ch))
}

/// Whether some key on the current layout reports `chord`'s key, so pressing it with the chord's
/// modifiers can match. Only punctuation is checked; letters, digits and named keys are always
/// there. With `by_place`, the app also matches by [`us_punctuation_at`], so US punctuation is
/// always there too.
#[cfg(windows)]
pub(super) fn is_typable(chord: &KeyChord, by_place: bool) -> bool {
	use windows::Win32::UI::Input::KeyboardAndMouse::{
		MAPVK_VK_TO_CHAR, MAPVK_VK_TO_VSC, MapVirtualKeyW, VK_OEM_1, VK_OEM_2, VK_OEM_3, VK_OEM_4, VK_OEM_5, VK_OEM_6,
		VK_OEM_7, VK_OEM_8, VK_OEM_102, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS,
	};
	// The keys wxMSW reports by the character they type rather than by a fixed key code.
	const OEM_KEYS: [u16; 13] = [
		VK_OEM_1.0,
		VK_OEM_PLUS.0,
		VK_OEM_COMMA.0,
		VK_OEM_MINUS.0,
		VK_OEM_PERIOD.0,
		VK_OEM_2.0,
		VK_OEM_3.0,
		VK_OEM_4.0,
		VK_OEM_5.0,
		VK_OEM_6.0,
		VK_OEM_7.0,
		VK_OEM_8.0,
		VK_OEM_102.0,
	];
	if !is_punctuation(chord) || (by_place && is_us_punctuation(chord)) {
		return true;
	}
	// The key code wxMSW gives the key: its unshifted character, nothing for a dead key, and for
	// a character outside Latin-1 the one the same key types on a US layout.
	let key_code = |vk: u16| {
		let mapped = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_CHAR) };
		if mapped == 0 || mapped & 0x8000_0000 != 0 {
			return None;
		}
		if mapped <= 0xFF {
			return i32::try_from(mapped).ok();
		}
		let us = us_punctuation_for_scan_code(unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC) })?;
		Some(us as i32)
	};
	OEM_KEYS
		.into_iter()
		.filter_map(key_code)
		.any(|code| chord.matches(code, chord.ctrl || chord.raw_ctrl, chord.alt, chord.shift))
}

/// Other platforms report a key by the character it types, so a chord there is never stranded on
/// a key the layout lacks.
#[cfg(not(windows))]
pub(super) const fn is_typable(_chord: &KeyChord, _by_place: bool) -> bool {
	true
}

#[cfg(windows)]
fn is_punctuation(chord: &KeyChord) -> bool {
	let mut chars = chord.key.chars();
	matches!((chars.next(), chars.next()), (Some(ch), None) if !ch.is_ascii_alphanumeric() && ch != ' ')
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Raw flags as Windows gives them: a repeat count of 1 and the scan code in bits 16 to 23.
	#[cfg(windows)]
	fn flags(scan_code: u32, extended: bool) -> u32 {
		1 | (scan_code << 16) | (u32::from(extended) << 24)
	}

	#[cfg(windows)]
	#[test]
	fn the_key_where_hungarian_has_e_acute_is_the_us_semicolon() {
		assert_eq!(us_punctuation_at(flags(0x27, false)), Some(';'));
		assert_eq!(us_punctuation_at(flags(0x28, false)), Some('\''));
		assert_eq!(us_punctuation_at(flags(0x1B, false)), Some(']'));
		assert_eq!(us_punctuation_at(flags(0x33, false)), Some(','));
	}

	#[cfg(windows)]
	#[test]
	fn letters_digits_and_the_keypad_are_not_punctuation() {
		assert_eq!(us_punctuation_at(flags(0x10, false)), None, "Q");
		assert_eq!(us_punctuation_at(flags(0x02, false)), None, "1");
		assert_eq!(us_punctuation_at(flags(0x35, true)), None, "the keypad's /");
		assert_eq!(us_punctuation_at(flags(0x56, false)), None, "the key a US keyboard lacks");
		assert_eq!(us_punctuation_at(0), None);
	}

	#[cfg(all(unix, not(target_os = "macos")))]
	#[test]
	fn linux_hardware_codes_are_evdev_codes_plus_eight() {
		assert_eq!(us_punctuation_at(59), Some(','));
		assert_eq!(us_punctuation_at(60), Some('.'));
		assert_eq!(us_punctuation_at(47), Some(';'));
		assert_eq!(us_punctuation_at(48), Some('\''));
		assert_eq!(us_punctuation_at(24), None, "Q");
		assert_eq!(us_punctuation_at(3), None);
	}

	#[cfg(target_os = "macos")]
	#[test]
	fn macos_never_looks_a_key_up() {
		assert_eq!(us_punctuation_at(0x2B), None);
	}

	#[cfg(windows)]
	#[test]
	fn letters_and_named_keys_are_never_checked() {
		assert!(is_typable(&KeyChord::new(true, false, false, "Q"), false));
		assert!(is_typable(&KeyChord::new(true, false, false, "F5"), false));
		assert!(is_typable(&KeyChord::new(false, false, false, "Space"), false));
	}

	#[cfg(windows)]
	#[test]
	fn a_comma_is_on_the_keyboard() {
		assert!(is_typable(&KeyChord::new(true, false, false, ","), false));
	}

	/// A US layout, which these tests assume, types `:` only with Shift, as a Finnish one does `;`.
	#[cfg(windows)]
	#[test]
	fn a_character_behind_shift_is_not() {
		assert!(!is_typable(&KeyChord::new(true, false, false, ":"), false));
		assert!(!is_typable(&KeyChord::new(true, false, false, ":"), true), "no US key types : without Shift");
	}

	#[test]
	fn us_punctuation_is_always_there_when_matched_by_place() {
		assert!(is_us_punctuation(&KeyChord::new(true, false, false, ";")));
		assert!(is_us_punctuation(&KeyChord::new(false, false, true, ",")));
		assert!(!is_us_punctuation(&KeyChord::new(true, false, false, ":")));
		assert!(!is_us_punctuation(&KeyChord::new(true, false, false, "F5")));
	}
}
