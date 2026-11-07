//! The message and question dialogs, for trying them by hand.
//!
//! "Follow request" asks with Accept, Reject and Cancel, and logs the answer. "Information" shows a message centred on the window.
use wx_utils::{ask, show_info};
use wxdragon::prelude::*;

fn main() {
	let _ = wxdragon::main(|_| {
		let frame = Frame::builder().with_title("Dialogs example").build();
		let panel = Panel::builder(&frame).build();
		let request = Button::builder(&panel).with_label("&Follow request").build();
		let info = Button::builder(&panel).with_label("&Information").build();
		let log = TextCtrl::builder(&panel)
			.with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly)
			.with_size(Size::new(400, 200))
			.build();
		request.on_click(move |_| {
			let answer =
				ask(&frame, "Alex (@alex@example.social) asked to follow you.", "Follow request", "&Accept", "&Reject");
			log.append_text(&format!("{answer:?}\n"));
			log.set_focus();
		});
		info.on_click(move |_| show_info(&frame, "Nothing new since you last looked.", "Information"));
		let sizer = BoxSizer::builder(Orientation::Vertical).build();
		sizer.add(&request, 0, SizerFlag::All, 10);
		sizer.add(&info, 0, SizerFlag::All, 10);
		sizer.add(&log, 1, SizerFlag::Expand | SizerFlag::All, 10);
		panel.set_sizer(sizer, true);
		frame.set_size(Size::new(450, 350));
		frame.show(true);
	});
}
