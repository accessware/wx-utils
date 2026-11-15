//! Tasks on the UI thread, for trying them by hand.
//!
//! "Count down" counts in the log once a second, then sums a range on a background thread. "Ticking dialog" opens a dialog whose task ticks in its title until the dialog closes, which cancels the task; the log says when that happens.
use std::time::Duration;

use wx_utils::tasks::{background_blocking, sleep, spawn_for};
use wxdragon::prelude::*;

/// Logs when the task holding it is dropped, as a cancelled task is.
struct Dropped(TextCtrl);

impl Drop for Dropped {
	fn drop(&mut self) {
		if self.0.is_valid() {
			self.0.append_text("The dialog's task was cancelled.\n");
		}
	}
}

fn main() {
	let _ = wxdragon::main(|_| {
		let frame = Frame::builder().with_title("Tasks example").build();
		let panel = Panel::builder(&frame).build();
		let count = Button::builder(&panel).with_label("&Count down").build();
		let ticking = Button::builder(&panel).with_label("&Ticking dialog").build();
		let log = TextCtrl::builder(&panel)
			.with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly)
			.with_size(Size::new(400, 200))
			.build();
		count.on_click(move |_| {
			spawn_for(&frame, async move {
				for n in (1..=3).rev() {
					log.append_text(&format!("{n}\n"));
					sleep(Duration::from_secs(1)).await;
				}
				let sum = background_blocking(|| (1..=50_000_000_u64).sum::<u64>()).await;
				log.append_text(&format!("The sum is {sum}.\n"));
			});
		});
		ticking.on_click(move |_| {
			let dialog = Dialog::builder(&frame, "Ticking").with_size(300, 100).build();
			spawn_for(&dialog, async move {
				let _dropped = Dropped(log);
				let mut tick = 0;
				loop {
					tick += 1;
					dialog.set_label(&format!("Ticking: {tick}"));
					sleep(Duration::from_millis(500)).await;
				}
			});
			dialog.on_close(move |_| dialog.destroy());
			dialog.show(true);
		});
		let sizer = BoxSizer::builder(Orientation::Vertical).build();
		sizer.add(&count, 0, SizerFlag::All, 8);
		sizer.add(&ticking, 0, SizerFlag::All, 8);
		sizer.add(&log, 1, SizerFlag::Expand | SizerFlag::All, 8);
		panel.set_sizer(sizer, true);
		frame.show(true);
	});
}
