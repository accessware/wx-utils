//! Hiding the main window to the notification area and bringing it back, and the icon there.
//!
//! [`Visibility`] hides and shows a frame along with whichever dialog the user was in, which
//! hiding a frame alone leaves on screen. [`Tray`] puts the app's icon in the notification area,
//! with a Show/Hide and Exit menu, and takes it out again when the frame closes, since an icon
//! left there keeps the process running.
//!
//! The dialog handling and telling whether the app is in front are Windows only. Elsewhere the
//! frame is shown and hidden on its own, and the app counts as in front when the frame has focus.

use std::{cell::RefCell, rc::Rc};

use patois::t;
use wxdragon::prelude::*;

const ID_SHOW_HIDE: i32 = ID_HIGHEST + 4900;
const ID_EXIT: i32 = ID_HIGHEST + 4901;

/// Whether a frame is shown, and the dialog over it with it.
pub struct Visibility {
	frame: Frame,
	/// The dialog hidden along with the frame, to show again when it comes back. Hiding a frame
	/// leaves the dialogs it owns on screen; only minimizing takes them too.
	#[cfg(windows)]
	hidden_dialog: std::cell::Cell<isize>,
}

impl Visibility {
	#[must_use]
	pub const fn new(frame: Frame) -> Self {
		Self {
			frame,
			#[cfg(windows)]
			hidden_dialog: std::cell::Cell::new(0),
		}
	}

	/// Hides the app when it's what the user is in, and otherwise brings it forward. For a
	/// global hotkey.
	pub fn toggle(&self) {
		if self.frame.is_shown() && self.is_active() {
			self.hide();
		} else {
			self.come_forward();
		}
	}

	/// Hides the app when it's shown, and otherwise brings it back. For the tray menu, which
	/// takes the foreground itself, so whether the app was in front can't be told any more.
	pub fn show_or_hide(&self) {
		if self.frame.is_shown() {
			self.hide();
		} else {
			self.come_forward();
		}
	}

	/// Brings the app to the front, shown and restored, onto the window the user was last in,
	/// such as an open dialog or an error message. For clicking the tray icon, or a second copy
	/// of the app starting.
	pub fn come_forward(&self) {
		// Found before the frame is shown, restored or raised, any of which makes the frame the
		// window last active and hides the dialog the user was in.
		#[cfg(windows)]
		let last_active = self.take_hidden_dialog().or_else(|| win32::last_active_popup(self.frame));
		self.frame.show(true);
		if self.frame.is_iconized() {
			self.frame.iconize(false);
		}
		self.frame.raise();
		#[cfg(windows)]
		if let Some(window) = last_active {
			win32::bring_forward(window);
			// Raised again now the app has the foreground, which it may not have had the first
			// time, as when a second copy asked it to come forward. Otherwise the frame can stay
			// behind the window that was in front, which then gets the focus back when the
			// dialog closes. Raising the frame takes the focus, so the dialog has it back after.
			self.frame.raise();
			win32::bring_forward(window);
		}
	}

	/// Hides the frame, and the dialog over it if there is one.
	pub fn hide(&self) {
		#[cfg(windows)]
		if let Some(dialog) = self.open_dialog() {
			win32::hide(dialog);
			self.hidden_dialog.set(dialog);
		}
		self.frame.show(false);
	}

	/// Asks the dialog open over the frame, shown or hidden with it, to close, as Escape would.
	/// Returns whether there was one to ask. Exiting from the tray asks until there are none
	/// left, since closing the frame under a modal dialog upsets wxWidgets.
	// Off Windows there's never a dialog to know of, which leaves nothing to look at.
	#[cfg_attr(not(windows), allow(clippy::unused_self, clippy::missing_const_for_fn, clippy::must_use_candidate))]
	pub fn close_dialog(&self) -> bool {
		#[cfg(windows)]
		if let Some(dialog) = self.take_hidden_dialog().or_else(|| self.open_dialog()) {
			win32::close(dialog);
			return true;
		}
		false
	}

	/// Whether a dialog is open over the frame, shown or hidden with it. Only Windows can tell, so
	/// elsewhere there never is one.
	#[must_use]
	#[cfg_attr(not(windows), allow(clippy::unused_self, clippy::missing_const_for_fn))]
	pub fn has_dialog(&self) -> bool {
		#[cfg(windows)]
		{
			self.hidden_dialog.get() != 0 || self.open_dialog().is_some()
		}
		#[cfg(not(windows))]
		{
			false
		}
	}

	/// Hides the frame, and the dialog over it, whenever it's minimized and `enabled` says to, as
	/// a "minimize to the tray" setting does. `hidden` runs first, such as to put up a [`Tray`]
	/// icon to bring it back with. `enabled` is asked each time, so the setting can change while
	/// the app runs.
	pub fn hide_when_minimized(self: &Rc<Self>, enabled: impl Fn() -> bool + 'static, hidden: impl Fn() + 'static) {
		let visibility = Rc::downgrade(self);
		let hide: Rc<dyn Fn()> = Rc::new(move || {
			let Some(visibility) = visibility.upgrade() else {
				return;
			};
			// A frame hidden while minimized still reports itself minimized, so without asking
			// whether it's shown, every idle event would hide it again.
			if visibility.frame.is_iconized() && visibility.frame.is_shown() && enabled() {
				hidden();
				visibility.hide();
			}
		});
		// Minimizing arrives as a size event on some platforms, and only shows by the next idle
		// one on others.
		let on_size = Rc::clone(&hide);
		self.frame.on_size(move |event| {
			on_size();
			event.skip(true);
		});
		self.frame.on_idle(move |event| {
			hide();
			event.skip(true);
		});
	}

	/// Whether the frame, or the dialog over it, is the window in front.
	#[must_use]
	pub fn is_active(&self) -> bool {
		#[cfg(windows)]
		{
			win32::foreground().is_some_and(|foreground| {
				foreground == win32::handle(self.frame) || Some(foreground) == win32::last_active_popup(self.frame)
			})
		}
		#[cfg(not(windows))]
		{
			self.frame.has_focus()
		}
	}

	/// The dialog the user was last in over the frame, if one is open.
	#[cfg(windows)]
	fn open_dialog(&self) -> Option<isize> {
		win32::last_active_popup(self.frame).filter(|popup| *popup != win32::handle(self.frame))
	}

	#[cfg(windows)]
	fn take_hidden_dialog(&self) -> Option<isize> {
		Some(self.hidden_dialog.replace(0)).filter(|dialog| *dialog != 0)
	}
}

type Callback = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// The app's icon in the notification area. Clicking it runs what
/// [`TrayBuilder::on_click`] says, and its menu has Show/Hide and Exit.
pub struct Tray {
	inner: Rc<Inner>,
}

struct Inner {
	icon: TaskBarIcon,
	/// Kept for as long as the icon shows it.
	menu: RefCell<Option<Menu>>,
	exit: Callback,
	/// The tooltip and picture, to put back after [`Tray::hide`].
	name: String,
	picture: Option<Picture>,
}

impl Inner {
	/// Shows the picture in the notification area, or the stock information icon without one.
	fn show(&self) {
		match &self.picture {
			Some(Picture::Bitmap(bitmap)) => {
				self.icon.set_icon(bitmap, &self.name);
			}
			Some(Picture::Bundle(bundle)) => {
				self.icon.set_icon_bundle(bundle, &self.name);
			}
			None => {
				if let Some(bitmap) =
					ArtProvider::get_bitmap(ArtId::Information, ArtClient::Menu, Some(Size::new(16, 16)))
				{
					self.icon.set_icon(&bitmap, &self.name);
				}
			}
		}
	}

	fn remove(&self) {
		if let Some(mut menu) = self.menu.borrow_mut().take() {
			menu.destroy_menu();
			self.icon.destroy();
		}
	}
}

impl Tray {
	/// Starts an icon for the app called `name`, which its tooltip shows.
	#[must_use]
	pub fn builder(name: &str) -> TrayBuilder {
		TrayBuilder { name: name.to_owned(), icon: None, show_hide: None, on_click: None, on_show_hide: None }
	}

	/// What the menu's Exit does. It can be set after the icon is up, once whatever it closes
	/// exists, and does nothing until it is.
	pub fn on_exit(&self, exit: impl Fn() + 'static) {
		*self.inner.exit.borrow_mut() = Some(Box::new(exit));
	}

	/// Takes the icon out of the notification area. It happens by itself when the frame closes,
	/// and is safe to do more than once.
	pub fn remove(&self) {
		self.inner.remove();
	}

	/// Takes the icon out of the notification area for now, to put back with
	/// [`show`](Self::show), for an icon only shown while the app is hidden. Unlike
	/// [`remove`](Self::remove), it's safe from the icon's own click or menu.
	pub fn hide(&self) {
		if self.menu_open() {
			self.inner.icon.remove_icon();
		}
	}

	/// Puts back an icon [`hide`](Self::hide) took out.
	pub fn show(&self) {
		if self.menu_open() {
			self.inner.show();
		}
	}

	/// Shows a notification from the icon for `timeout` milliseconds, with wxWidgets' balloon
	/// `flags`, such as `wxICON_INFORMATION`. Only Windows shows one, and only while the icon is
	/// up.
	pub fn show_balloon(&self, title: &str, text: &str, timeout: u32, flags: i32) {
		if self.menu_open() {
			self.inner.icon.show_balloon(title, text, timeout, flags, None);
		}
	}

	/// Whether the icon is still up, rather than removed.
	fn menu_open(&self) -> bool {
		self.inner.menu.borrow().is_some()
	}
}

/// What the icon shows: one picture, or several sizes to pick from.
enum Picture {
	Bitmap(Bitmap),
	Bundle(BitmapBundle),
}

/// Sets up a [`Tray`] before it's shown.
pub struct TrayBuilder {
	name: String,
	icon: Option<Picture>,
	/// The Show/Hide item's label and help, when the app gives its own.
	show_hide: Option<(String, String)>,
	on_click: Option<Box<dyn Fn()>>,
	on_show_hide: Option<Box<dyn Fn()>>,
}

impl TrayBuilder {
	/// The icon to show, at the notification area's size, usually 16 pixels square. Without one,
	/// the stock information icon stands in.
	#[must_use]
	pub fn icon(mut self, icon: Bitmap) -> Self {
		self.icon = Some(Picture::Bitmap(icon));
		self
	}

	/// The icon to show, from a bundle of sizes, so it stays sharp at any display scaling.
	#[must_use]
	pub fn icon_bundle(mut self, icon: BitmapBundle) -> Self {
		self.icon = Some(Picture::Bundle(icon));
		self
	}

	/// Labels the Show/Hide item `label`, with `help` for the status bar, such as "&Restore" for
	/// an icon only shown while the app is hidden.
	#[must_use]
	pub fn show_hide_label(mut self, label: &str, help: &str) -> Self {
		self.show_hide = Some((label.to_owned(), help.to_owned()));
		self
	}

	/// What clicking the icon does, usually [`Visibility::come_forward`].
	#[must_use]
	pub fn on_click(mut self, click: impl Fn() + 'static) -> Self {
		self.on_click = Some(Box::new(click));
		self
	}

	/// What the menu's Show/Hide does, usually [`Visibility::show_or_hide`].
	#[must_use]
	pub fn on_show_hide(mut self, show_hide: impl Fn() + 'static) -> Self {
		self.on_show_hide = Some(Box::new(show_hide));
		self
	}

	/// Puts the icon in the notification area, and has it taken out again when `frame` closes.
	#[must_use]
	pub fn build(self, frame: &Frame) -> Tray {
		let (show_hide_label, show_hide_help) =
			self.show_hide.unwrap_or_else(|| (t("&Show/Hide"), t("Show or hide the window")));
		let mut menu = Menu::builder()
			.append_item(ID_SHOW_HIDE, &show_hide_label, &show_hide_help)
			.append_separator()
			.append_item(ID_EXIT, &t("E&xit"), &t("Close the app"))
			.build();
		let icon = TaskBarIcon::builder().build();
		icon.set_popup_menu(&mut menu);
		let exit: Callback = Rc::default();
		let menu_exit = Rc::clone(&exit);
		let show_hide = self.on_show_hide;
		icon.on_menu(move |event| match event.get_id() {
			ID_SHOW_HIDE => {
				if let Some(show_hide) = &show_hide {
					show_hide();
				}
			}
			ID_EXIT => {
				if let Some(exit) = &*menu_exit.borrow() {
					exit();
				}
			}
			_ => {}
		});
		if let Some(click) = self.on_click {
			// Windows reports a click as the button coming up, and Linux only as it going down. A
			// macOS status item has its menu, and no clicks.
			#[cfg(windows)]
			icon.on_left_up(move |_| click());
			#[cfg(target_os = "linux")]
			icon.on_left_down(move |_| click());
			#[cfg(not(any(windows, target_os = "linux")))]
			drop(click);
		}
		let inner = Rc::new(Inner { icon, menu: RefCell::new(Some(menu)), exit, name: self.name, picture: self.icon });
		inner.show();
		let closing = Rc::downgrade(&inner);
		frame.on_close(move |event| {
			// The icon would otherwise keep the app running once the window is gone.
			if let Some(inner) = closing.upgrade() {
				inner.remove();
			}
			event.skip(true);
		});
		Tray { inner }
	}
}

/// The few Windows calls the rest of this needs, with window handles as plain numbers so they
/// can sit in a `Cell`.
#[cfg(windows)]
mod win32 {
	use windows::Win32::{
		Foundation::{HWND, LPARAM, WPARAM},
		UI::WindowsAndMessaging::{
			GetForegroundWindow, GetLastActivePopup, PostMessageW, SW_HIDE, SW_SHOW, SetForegroundWindow, ShowWindow,
			WM_CLOSE,
		},
	};
	use wxdragon::prelude::*;

	pub fn handle(frame: Frame) -> isize {
		frame.get_handle() as isize
	}

	/// The window `frame` or one of its dialogs was last active in; `frame` itself when no dialog
	/// is open.
	pub fn last_active_popup(frame: Frame) -> Option<isize> {
		let frame = handle(frame);
		// SAFETY: `frame` is the live main window's handle.
		(frame != 0).then(|| unsafe { GetLastActivePopup(HWND(frame as _)) }.0 as isize)
	}

	pub fn foreground() -> Option<isize> {
		// SAFETY: takes nothing and only reads which window is in front.
		let window = unsafe { GetForegroundWindow() }.0 as isize;
		(window != 0).then_some(window)
	}

	/// Asks `window` to close once its event loop gets to it, as its close button would.
	pub fn close(window: isize) {
		// SAFETY: a handle Windows gave out for one of this process's windows.
		let _ = unsafe { PostMessageW(Some(HWND(window as _)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
	}

	pub fn hide(window: isize) {
		// SAFETY: a handle Windows gave out for one of this process's windows.
		let _ = unsafe { ShowWindow(HWND(window as _), SW_HIDE) };
	}

	/// Shows `window` and puts it in front. Windows refuses the foreground to a process the user
	/// isn't using, but a hotkey, the tray, and a second copy of the app all count as using it.
	pub fn bring_forward(window: isize) {
		// SAFETY: a handle Windows gave out for one of this process's windows.
		unsafe {
			let _ = ShowWindow(HWND(window as _), SW_SHOW);
			let _ = SetForegroundWindow(HWND(window as _));
		}
	}
}
