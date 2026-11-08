//! The message and question dialogs, for trying them by hand.
//!
//! "Follow request" asks with Accept, Reject and Cancel, and logs the answer. "Information" shows a message centred on the window. "Text" shows a long text in a read-only box. "Form" lays out a dialog with `Form` and logs what was chosen.
use std::fmt::Write as _;

use wx_utils::{
	add_ok_cancel_footer, ask, build_ok_cancel_buttons, form::Form, natural_slider_keys, show_info, show_text,
};
use wxdragon::prelude::*;

fn main() {
	let _ = wxdragon::main(|_| {
		let frame = Frame::builder().with_title("Dialogs example").build();
		let panel = Panel::builder(&frame).build();
		let request = Button::builder(&panel).with_label("&Follow request").build();
		let info = Button::builder(&panel).with_label("&Information").build();
		let text = Button::builder(&panel).with_label("&Text").build();
		let form_button = Button::builder(&panel).with_label("F&orm").build();
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
		text.on_click(move |_| {
			let mut lines = String::new();
			for n in 1..=2000 {
				let _ = writeln!(lines, "12:00:{:02} INFO line {n} of a long log", n % 60);
			}
			show_text(&frame, "Log", &lines);
		});
		form_button.on_click(move |_| {
			if let Some(chosen) = show_form(&frame) {
				log.append_text(&format!("{chosen}\n"));
			}
			log.set_focus();
		});
		let sizer = BoxSizer::builder(Orientation::Vertical).build();
		sizer.add(&request, 0, SizerFlag::All, 10);
		sizer.add(&info, 0, SizerFlag::All, 10);
		sizer.add(&text, 0, SizerFlag::All, 10);
		sizer.add(&form_button, 0, SizerFlag::All, 10);
		sizer.add(&log, 1, SizerFlag::Expand | SizerFlag::All, 10);
		panel.set_sizer(sizer, true);
		frame.set_size(Size::new(450, 350));
		frame.show(true);
	});
}

fn show_form(parent: &Frame) -> Option<String> {
	let dialog = Dialog::builder(parent, "Speech").build();
	let form = Form::new(dialog);
	let voice = form.labeled("&Voice:", |parent| {
		let choice = Choice::builder(parent).build();
		for name in ["System default", "David", "Zira"] {
			choice.append(name);
		}
		choice.set_selection(0);
		choice
	});
	let rate = form.labeled("&Rate:", |parent| {
		Slider::builder(parent).with_value(50).with_min_value(0).with_max_value(100).build()
	});
	natural_slider_keys(rate, |_| {});
	let notes = form.labeled_above("&Notes:", true, |parent| {
		TextCtrl::builder(parent).with_style(TextCtrlStyle::MultiLine).with_size(Size::new(300, 80)).build()
	});
	let book_only = form.check_box("For this &book only");
	let (ok, cancel) = build_ok_cancel_buttons(&dialog, "OK");
	add_ok_cancel_footer(form.sizer(), ok, cancel);
	dialog.set_sizer_and_fit(form.sizer(), true);
	dialog.centre();
	let chosen = (dialog.show_modal() == ID_OK).then(|| {
		format!(
			"voice {}, rate {}, notes {:?}, book only {}",
			voice.get_string_selection().unwrap_or_default(),
			rate.get_value(),
			notes.get_value(),
			book_only.is_checked()
		)
	});
	dialog.destroy();
	chosen
}
