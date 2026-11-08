//! Sliders whose arrow keys go the way they point.

use std::rc::Rc;

use wxdragon::prelude::*;

/// Makes `slider`'s keys move it the way people expect, and calls `on_change` with each new value.
///
/// A Windows slider is a trackbar, which treats Up and Page Up as moving toward the top of a vertical slider, so on a horizontal one they lower the value: a screen reader user pressing Up to speed up a voice slows it down. With this, Up and Right add one, Down and Left take one away, Page Up and Page Down move a tenth of the range, and Home and End go to the ends, on every platform.
///
/// Setting the value from code fires no slider event, so `on_change` is where anything that follows the slider, such as a number beside it, gets the new value. It isn't called when the slider is already at the end it's pushed against.
pub fn natural_slider_keys(slider: Slider, on_change: impl Fn(i32) + 'static) {
	let on_change = Rc::new(on_change);
	slider.bind_internal(EventType::CHAR, move |event| {
		let key = event.get_key_code().unwrap_or(0);
		let Some(target) = target_value(key, slider.value(), slider.min(), slider.max()) else {
			event.skip(true);
			return;
		};
		event.skip(false);
		if target != slider.value() {
			slider.set_value(target);
			on_change(target);
		}
	});
}

/// Where `key` moves a slider at `value` in `min..=max`, or `None` for a key that doesn't move it.
fn target_value(key: i32, value: i32, min: i32, max: i32) -> Option<i32> {
	let page = ((max - min) / 10).max(1);
	let target = match key {
		WXK_UP | WXK_RIGHT | WXK_NUMPAD_UP | WXK_NUMPAD_RIGHT => value.saturating_add(1),
		WXK_DOWN | WXK_LEFT | WXK_NUMPAD_DOWN | WXK_NUMPAD_LEFT => value.saturating_sub(1),
		WXK_PAGEUP | WXK_NUMPAD_PAGEUP => value.saturating_add(page),
		WXK_PAGEDOWN | WXK_NUMPAD_PAGEDOWN => value.saturating_sub(page),
		WXK_HOME | WXK_NUMPAD_HOME => min,
		WXK_END | WXK_NUMPAD_END => max,
		_ => return None,
	};
	Some(target.clamp(min, max))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn up_and_right_raise_the_value() {
		assert_eq!(target_value(WXK_UP, 50, 0, 100), Some(51));
		assert_eq!(target_value(WXK_RIGHT, 50, 0, 100), Some(51));
		assert_eq!(target_value(WXK_DOWN, 50, 0, 100), Some(49));
		assert_eq!(target_value(WXK_LEFT, 50, 0, 100), Some(49));
	}

	#[test]
	fn page_keys_move_a_tenth_of_the_range() {
		assert_eq!(target_value(WXK_PAGEUP, 50, 0, 100), Some(60));
		assert_eq!(target_value(WXK_PAGEDOWN, 50, 0, 100), Some(40));
		assert_eq!(target_value(WXK_PAGEUP, 0, 0, 5), Some(1));
		assert_eq!(target_value(WXK_PAGEDOWN, 0, -20, 20), Some(-4));
	}

	#[test]
	fn the_value_stays_in_range() {
		assert_eq!(target_value(WXK_UP, 100, 0, 100), Some(100));
		assert_eq!(target_value(WXK_PAGEDOWN, 3, 0, 100), Some(0));
		assert_eq!(target_value(WXK_HOME, 70, 10, 90), Some(10));
		assert_eq!(target_value(WXK_END, 70, 10, 90), Some(90));
	}

	#[test]
	fn other_keys_are_left_alone() {
		assert_eq!(target_value(i32::from(b'a'), 50, 0, 100), None);
		assert_eq!(target_value(WXK_TAB, 50, 0, 100), None);
		assert_eq!(target_value(WXK_RETURN, 50, 0, 100), None);
	}
}
