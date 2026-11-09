//! Putting a frame back where the user left it.
//!
//! The default and minimum sizes are device-independent pixels, so they mean the same at any
//! scaling. The saved rectangle is physical pixels, as wxWidgets reports them, and is restored as
//! it was, since converting it would move the window away from where the user put it. Where the
//! app keeps it is up to the app: [`capture`] gives it something to save, and [`restore`] takes it
//! back.

use wxdragon::prelude::*;

/// Physical pixels, small enough to sample inside the title bar rather than measure it.
const TITLE_BAR_DEPTH: i32 = 8;
const TITLE_BAR_INSET: i32 = 24;

/// What to save about a frame between runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowGeometry {
	/// Where the frame was and how big, or `None` when it was maximized or minimized. A maximized
	/// frame covers the whole display and a minimized one is parked off-screen, so either would lose
	/// the size to go back to; keep the rectangle saved before instead.
	pub rect: Option<Rect>,
	pub maximized: bool,
}

/// Sets `frame`'s size and smallest size, both in device-independent pixels.
///
/// Call it after building the frame and before showing it, so the size is scaled for the display it
/// opens on and there's no visible resize.
pub fn apply_defaults(frame: &Frame, size: Size, min_size: Size) {
	frame.set_min_size(frame.from_dip(min_size));
	frame.set_size(frame.from_dip(size));
}

/// Puts `frame` where `saved` says, while it's still hidden so the move isn't a visible jump.
///
/// A rectangle partly off the left or top edge is used as it is: Windows allows it, and it's where
/// the user left the window. One whose monitor is gone, because it was unplugged or the laptop was
/// undocked, keeps its size, which was a deliberate choice, and is centred on the primary display. A
/// frame with nothing saved is centred.
pub fn restore(frame: &Frame, saved: WindowGeometry) {
	match saved.rect.filter(|rect| rect.width > 0 && rect.height > 0) {
		Some(rect) if is_reachable(rect) => frame.set_size_with_pos(rect.x, rect.y, rect.width, rect.height),
		Some(rect) => {
			frame.set_size(fit_to_primary(Size::new(rect.width, rect.height)));
			frame.centre();
		}
		None => frame.centre(),
	}
	if saved.maximized {
		frame.maximize(true);
	}
}

/// What to save about `frame`, usually as it closes.
#[must_use]
pub fn capture(frame: &Frame) -> WindowGeometry {
	let maximized = frame.is_maximized();
	if maximized || frame.is_iconized() {
		return WindowGeometry { rect: None, maximized };
	}
	let position = frame.get_position();
	let size = frame.get_size();
	let rect = (size.width > 0 && size.height > 0).then(|| Rect::new(position.x, position.y, size.width, size.height));
	WindowGeometry { rect, maximized }
}

/// Whether a display shows some of the title bar, sampled at three points along it, since a window
/// nudged off one side can still be dragged back.
fn is_reachable(rect: Rect) -> bool {
	let y = rect.y + TITLE_BAR_DEPTH;
	[rect.x + TITLE_BAR_INSET, rect.x + rect.width / 2, rect.x + rect.width - TITLE_BAR_INSET]
		.into_iter()
		.any(|x| Display::from_point(Point::new(x, y)).is_some())
}

fn fit_to_primary(size: Size) -> Size {
	let Some(area) = Display::new(0).map(|display| display.client_area()) else {
		return size;
	};
	Size::new(size.width.min(area.width), size.height.min(area.height))
}
