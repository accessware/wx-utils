//! Laying out a dialog's controls: labels beside the controls they name, lined up in two columns, with [`dialog_padding`](crate::dialog_padding) between everything.

use std::cell::RefCell;

use wxdragon::prelude::*;

use crate::dialog_padding;

/// Builds a dialog's or panel's controls top to bottom.
///
/// Each control is built by a closure the form calls after creating its label, so the label always comes first: a screen reader names a control after the label created just before it, and the tab order follows the order controls are created. Labelled rows in a row share a grid, so their controls line up; a full-width row ends the grid, and the next labelled row starts a new one.
///
/// ```no_run
/// use wx_utils::{add_ok_cancel_footer, build_ok_cancel_buttons, form::Form};
/// use wxdragon::prelude::*;
///
/// fn show(parent: &Frame) {
///     let dialog = Dialog::builder(parent, "Speech").build();
///     let form = Form::new(dialog);
///     let voice = form.labeled("&Voice:", |parent| Choice::builder(parent).build());
///     let rate = form.labeled("&Rate:", |parent| Slider::builder(parent).build());
///     let only_this_book = form.check_box("For this &book only");
///     let (ok, cancel) = build_ok_cancel_buttons(&dialog, "OK");
///     add_ok_cancel_footer(form.sizer(), ok, cancel);
///     dialog.set_sizer_and_fit(form.sizer(), true);
/// }
/// ```
pub struct Form<P> {
	parent: P,
	sizer: BoxSizer,
	padding: i32,
	/// Where labelled rows go, until a full-width row ends it.
	grid: RefCell<Option<FlexGridSizer>>,
}

impl<P: WxWidget + Copy> Form<P> {
	/// A form for `parent`, usually a dialog or a tab's panel.
	#[must_use]
	pub fn new(parent: P) -> Self {
		let padding = dialog_padding(&parent);
		Self { parent, sizer: BoxSizer::builder(Orientation::Vertical).build(), padding, grid: RefCell::new(None) }
	}

	/// A label, then the control `build` makes for it, side by side. The control takes the row's spare width.
	pub fn labeled<C: WxWidget>(&self, label: &str, build: impl FnOnce(&P) -> C) -> C {
		let label = StaticText::builder(&self.parent).with_label(label).build();
		let control = build(&self.parent);
		let mut grid = self.grid.borrow_mut();
		let grid = grid.get_or_insert_with(|| {
			let grid = FlexGridSizer::builder(0, 2).with_hgap(self.padding).with_vgap(self.padding / 2).build();
			grid.add_growable_col(1, 1);
			self.sizer.add_sizer(
				&grid,
				0,
				SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
				self.padding,
			);
			grid
		});
		grid.add(&label, 0, SizerFlag::AlignCenterVertical, 0);
		grid.add(&control, 1, SizerFlag::Expand, 0);
		control
	}

	/// A label on a line of its own, then the control `build` makes for it across the whole width, such as a list or a multi-line text box. With `grow`, the control takes the form's spare height.
	pub fn labeled_above<C: WxWidget>(&self, label: &str, grow: bool, build: impl FnOnce(&P) -> C) -> C {
		let label = StaticText::builder(&self.parent).with_label(label).build();
		let control = build(&self.parent);
		self.end_grid();
		self.sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, self.padding);
		self.sizer.add(
			&control,
			i32::from(grow),
			SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
			self.padding / 2,
		);
		control
	}

	/// A control across the whole width, such as a check box or a button, which `build` makes.
	pub fn add<C: WxWidget>(&self, build: impl FnOnce(&P) -> C) -> C {
		let control = build(&self.parent);
		self.end_grid();
		self.sizer.add(&control, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, self.padding);
		control
	}

	/// A check box across the whole width, unchecked.
	pub fn check_box(&self, label: &str) -> CheckBox {
		self.add(|parent| CheckBox::builder(parent).with_label(label).build())
	}

	/// The sizer holding the form, for a footer such as [`add_ok_cancel_footer`](crate::add_ok_cancel_footer), and then the parent's `set_sizer_and_fit`.
	#[must_use]
	pub const fn sizer(&self) -> BoxSizer {
		self.sizer
	}

	/// So the next labelled row starts a grid below what was just added, rather than joining the one above it.
	fn end_grid(&self) {
		self.grid.borrow_mut().take();
	}
}
