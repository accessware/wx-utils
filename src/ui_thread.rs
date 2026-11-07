//! Running code on the UI thread from another thread.

/// Runs `callback` on the UI thread, soon.
///
/// [`wxdragon::call_after`] only queues the callback, and an event loop with nothing else to do doesn't look at the queue until the next event, such as the mouse moving. This also wakes the loop, so a worker thread's result shows up at once.
pub fn post_to_ui(callback: impl FnOnce() + Send + 'static) {
	wxdragon::call_after(Box::new(callback));
	wxdragon::wake_up_idle();
}
