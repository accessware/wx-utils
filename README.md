# wx-utils

Rust utilities for building desktop apps with wxDragon.

## Features

### AboutBoxBuilder

A fluent interface for creating and showing native wxAboutBox dialogs:

* No manual FFI: you don't create, destroy, or cast `AboutDialogInfo` pointers yourself.
* Every field you set is converted to a C string for you.
* Setters take `impl Into<String>`, so translation macros and plain strings both work.

### Dialog button footers

Build and lay out a standard OK/Cancel row in two steps, which are meant to be used together:

* `build_ok_cancel_buttons` constructs the pair and wires up the behavior that's the same on every platform: `ID_OK` and `ID_CANCEL`, Escape cancels, and Enter confirms. It doesn't decide the on-screen order.
* `add_ok_cancel_footer` (and `add_single_button_footer`, for a single dismiss button) then lays the buttons out with a native `wxStdDialogButtonSizer`. That sizer is what enforces the platform's order (Cancel then OK on macOS, OK then Cancel on Windows), whatever order you pass the buttons in.
* `bind_enter_confirms` submits a dialog when the user presses Enter in a text or spin control.
* `build_yes_no_buttons` and `add_yes_no_footer` are the Yes/No counterparts, and `confirm` is the whole dialog in one call.
* `ask` is a question with two buttons you label and Cancel, such as Accept, Reject and Cancel. It returns an `Answer`.
* `dialog_padding` returns the padding these functions use, scaled for the display that the given window is on, so your own sizers can match.

Each `build_*_buttons` function has an `_on` variant that parents the buttons to a `Panel` (or any other window) instead of to the dialog itself, for dialogs whose content lives in a panel.

The Cancel label is translated automatically (see [Translation](#translation)). You only supply the OK label, because it varies by dialog, for example "OK" or "Go".

### Laying out a form

`form::Form` adds a dialog's or panel's controls top to bottom, with `dialog_padding` between everything:

* `labeled` puts a label beside the control it names. Labelled rows in a row line up in two columns.
* `labeled_above` puts the label on a line of its own, above a control that needs the whole width, such as a list or a multi-line text box.
* `add` and `check_box` add a control across the whole width.
* `sizer` gives the layout to a footer such as `add_ok_cancel_footer`, and then to the parent's `set_sizer_and_fit`.

You build each control in a closure the form calls after it creates the label. A screen reader names a control after the label created just before it, and the tab order follows the order of creation, so both come out right without extra work.

```rust
let form = Form::new(dialog);
let voice = form.labeled(&t("&Voice:"), |parent| Choice::builder(parent).build());
let rate = form.labeled(&t("&Rate:"), |parent| Slider::builder(parent).build());
let book_only = form.check_box(&t("For this &book only"));
```

### Sliders

A Windows slider lowers its value when you press Up or Page Up, because it treats every slider as if it stood upright. `natural_slider_keys` makes Up and Right raise it, Down and Left lower it, Page Up and Page Down move a tenth of the range, and Home and End go to the ends, on every platform. Setting a slider's value from code fires no event, so the closure you pass gets each new value, for anything that follows the slider.

```rust
let rate = Slider::builder(&dialog).with_value(50).with_min_value(0).with_max_value(100).build();
natural_slider_keys(rate, |value| log::debug!("rate {value}"));
```

### Why `confirm` instead of a Yes/No `MessageDialog`

On Windows, `MessageDialogStyle::YesNo` is the native task dialog, and the *operating system* labels its buttons, in the system language. An app translated into French and running on English Windows shows a French question above English Yes and No buttons. `confirm` builds real `Button`s labeled through patois, so they read in the app's own language like the rest of the dialog.

### Keyboard shortcuts

`shortcuts::prompt_for_shortcuts` is a complete Customize Keyboard Shortcuts dialog. It has a tab for each group of actions, a list of those actions with their current bindings, Set, Clear, Reset, and Reset All buttons, and conflict detection. Its capture dialog records whatever key combination you press, and announces it to a screen reader as you type.

Your app keeps its own action type and its own storage. Implement `shortcuts::ShortcutModel` over whatever your config already looks like, and the dialog does the rest, handing back an edited copy when the user confirms.

`ShortcutModel::tab_scope` needs the most care. It says when each tab's shortcuts are active, and two chords only conflict if their tabs can be active at the same time:

* `TabScope::Mode(n)`: active while the app is in mode `n`. Tabs in the same mode are views of one keymap, so a chord has to be unique across them, which is what tabs grouped by category need. Tabs in different modes never conflict, so the same key can mean different things in each, which is what tabs for separate input modes need.
* `TabScope::Global`: system-wide hotkeys, active whether or not the window has focus. They fire even over the app's own window, so they conflict with every other tab. The dialog requires Ctrl, Alt, or Win on each of these chords, and adds a Win checkbox to the capture dialog.

The scope also decides what Reset All covers: every tab in the same mode, or only the Global tab itself.

A chord names its key by the character it types, so on another layout a punctuation shortcut can sit on a key that types something else. On a Hungarian keyboard, the key where a US one has `;` types `é`, and GTK reports `Shift+,` as `<` on any layout. `shortcuts::us_punctuation_at` takes a key event's `get_raw_key_flags()` and gives the punctuation that key types on a US keyboard, on Windows and Linux. Match on it when a key's own character matches no shortcut, and return `true` from `ShortcutModel::matches_by_place`, so the list stops marking those chords as missing from the layout.

This is behind the `shortcuts` feature, which is off by default because it pulls in [live-region](https://github.com/accessware/live-region) for the announcements. The chord type is [key-chord](https://github.com/accessware/key-chord), re-exported as `shortcuts::KeyChord`.

### Global hotkeys

`global_hotkeys::GlobalHotkeys::register` registers chords system-wide, each paired with a value you choose, and calls you back with that value when one is pressed. Dropping the returned handle unregisters them all. It also returns the chords it couldn't register, usually because another program holds them, so you can tell the user instead of leaving a hotkey that silently does nothing.

The callback runs on the hotkey thread, so pass the value along (for example, down a channel that your UI thread drains) instead of touching widgets from it. Holding a hotkey down repeats it, like any other key.

This is behind the `global-hotkeys` feature. Only Windows is implemented. On other platforms, every chord comes back unregistered.

### Menus

* `menu_label` joins an item's text and its shortcut into the `"Open	Ctrl+O"` form that wx expects. An empty shortcut gives back a bare label instead of one with a trailing tab.
* `set_menu_item_label` relabels an existing item in a `MenuBar`, and does nothing if the ID isn't there, so you can relabel a menu that varies by platform in one pass.

### Durations

`format_duration_seconds` and `format_duration_ms` write a duration as a localized list of its nonzero parts, such as "1 hour, 5 minutes, 3 seconds". They use words instead of `1:05:03` because screen readers read this text aloud, and a colon-separated form is likely to be read as a time of day.

### Prompts and messages

* `prompt_number` shows a dialog with a labeled spin control, and returns the value entered, or `None` if the user cancels.
* `prompt_text` shows a single-line text entry dialog, and returns the trimmed value, or `None` if the user cancels or leaves it blank.
* `show_error`, `show_warning` and `show_info` show a modal message dialog with the matching icon, centred on its parent.
* `show_text` shows text in a read-only box with a Close button, such as a report before it's sent or a document's details. The box has the focus, so a screen reader user can read it line by line.

### Running code on the UI thread

`post_to_ui` runs a closure on the UI thread from any other thread. `wxdragon::call_after` alone only queues it, and an idle event loop doesn't run the queue until the next event, such as the mouse moving. `post_to_ui` also wakes the loop, so a worker thread's result shows up at once.

### Progress window

`progress::run_with_progress` runs work on a worker thread behind a native progress window. When the work returns and the progress window is gone, it calls you back on the UI thread with the result, and whether the user pressed Cancel. By then the application's windows are enabled again, so the callback can open windows and move focus. The work gets a `Progress` handle:

* `set` moves the gauge, or makes it pulse when the total is unknown.
* `set_message` replaces the text in the window.
* `is_cancelled` and `cancel_flag` tell the work that the user pressed Cancel.
* `ask` shows a dialog of your own, such as `confirm`, over the progress window, and hands its result back to the work.

One progress window shows at a time. A run started while another one shows waits until that one has ended.

`progress::run_without_estimate` is the same, without the estimate of the time remaining. Use it for work whose length can't be known, such as a single network request, where the window would otherwise show "Remaining time: unknown" the whole time.

To try it, run `cargo run --example progress`. The example shows fake downloads.

## Translation

wx-utils is marked `[package.metadata.patois] translatable = true`. If your app uses [patois](https://github.com/accessware/patois) and calls `patois_build::gen_pot` over your workspace, wx-utils's own translatable strings, such as "Cancel", are added to your app's catalog automatically. You don't need to set anything up. Your app must use the same version of patois as wx-utils: patois 0.3 for wx-utils 0.5. With another version, Cargo builds two copies of patois, and these strings stay in English.

## Quick start

```rust
use wx_utils::AboutBoxBuilder;
use wxdragon::prelude::*;

// Inside a Frame event handler or menu callback
pub fn on_about(parent: &Frame) {
	AboutBoxBuilder::new(parent)
		.name("Paperback")
		.version(env!("CARGO_PKG_VERSION"))
		.description("An accessible, lightweight, fast ebook and document reader.")
		.copyright("Copyright (C) 2025-2026 Quin Gillespie")
		.website("https://paperback.dev")
		.add_developer("Quin Gillespie")
		.show();
}
```

```rust
use wx_utils::confirm;
use wxdragon::prelude::*;

pub fn confirm_delete(parent: &Frame) -> bool {
	confirm(parent, "Delete this file permanently?", "Delete File")
}
```

For a dialog of your own, the button helpers lay out the footer:

```rust
use wx_utils::{add_ok_cancel_footer, build_ok_cancel_buttons, dialog_padding};
use wxdragon::prelude::*;

pub fn prompt_for_name(parent: &Frame) -> Option<String> {
	let dialog = Dialog::builder(parent, "Rename").build();
	let padding = dialog_padding(&dialog);
	let entry = TextCtrl::builder(&dialog).build();
	let (ok_button, cancel_button) = build_ok_cancel_buttons(&dialog, "Rename");
	let content = BoxSizer::builder(Orientation::Vertical).build();
	content.add(&entry, 0, SizerFlag::Expand | SizerFlag::All, padding);
	add_ok_cancel_footer(content, ok_button, cancel_button);
	dialog.set_sizer_and_fit(content, true);
	dialog.centre();
	if dialog.show_modal() == ID_OK { Some(entry.get_value()) } else { None }
}
```

## Planned

* ReadmeOpener: open local or remote readmes in the user's default browser.
* Automatic help menu generation.
