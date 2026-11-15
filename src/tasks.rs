//! Async tasks that run on the UI thread, and the work they hand to background threads.

use std::{
	cell::RefCell,
	collections::{HashMap, hash_map::Entry},
	panic::resume_unwind,
	pin::Pin,
	sync::LazyLock,
	task::{Context, Poll},
	time::Duration,
};

use async_task::Runnable;
pub use async_task::Task;
use tokio::{
	runtime::{Builder, Runtime},
	task::JoinHandle,
	time,
};
use wxdragon::{event::EventType, prelude::*};

use crate::post_to_ui;

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
	Builder::new_multi_thread()
		.enable_all()
		.thread_name("wx-utils-background")
		.build()
		.expect("the background runtime's threads should start")
});

thread_local! {
	/// The tasks started by `spawn_for`, by the window they belong to.
	static OWNED: RefCell<HashMap<usize, Vec<Task<()>>>> = RefCell::new(HashMap::new());
}

/// Runs `future` on the UI thread, so it can keep widgets, `Rc`s and other values that can't
/// cross threads between its awaits.
///
/// Call this on the UI thread. Dropping the returned [`Task`] cancels the future, so call
/// [`Task::detach`] to let it run on its own, or use [`spawn_for`] to tie it to a window.
pub fn spawn_local<F>(future: F) -> Task<F::Output>
where
	F: Future + 'static,
	F::Output: 'static,
{
	let (runnable, task) = async_task::spawn_local(future, schedule);
	runnable.schedule();
	task
}

// Every poll, the first included, is a callback of its own rather than part of whoever spawned or
// woke the task, which may still be borrowing what the task uses.
fn schedule(runnable: Runnable) {
	post_to_ui(move || {
		runnable.run();
	});
}

/// Runs `future` on the UI thread like [`spawn_local`], and cancels it when `window` is destroyed,
/// so it can use the window and its children without checking that they still exist.
///
/// Cancelling drops the future at the await it is waiting on, which also cancels a
/// [`background`] future it is waiting for.
pub fn spawn_for<W, F>(window: &W, future: F)
where
	W: WxWidget + WxEvtHandler,
	F: Future<Output = ()> + 'static,
{
	let window_ptr = window.handle_ptr();
	if window_ptr.is_null() {
		return;
	}
	let task = spawn_local(future);
	let first = OWNED.with_borrow_mut(|owned| match owned.entry(window_ptr as usize) {
		Entry::Occupied(mut tasks) => {
			tasks.get_mut().retain(|task| !task.is_finished());
			tasks.get_mut().push(task);
			false
		}
		Entry::Vacant(slot) => {
			slot.insert(vec![task]);
			true
		}
	});
	if first {
		window.bind_internal(EventType::DESTROY, move |event| {
			// Destroy events from the window's children reach it too.
			if event.event_object_ptr() == window_ptr {
				// Dropped after the borrow ends, since dropping a future may start another task.
				drop(OWNED.with_borrow_mut(|owned| owned.remove(&(window_ptr as usize))));
			}
			event.skip(true);
		});
	}
}

/// Work running on a background thread, which a task awaits for its output.
///
/// Dropping it cancels the work at its next await. Blocking work from [`background_blocking`]
/// can't be stopped part way, so it finishes and its output is dropped.
#[must_use = "dropping it cancels the work"]
pub struct Background<T>(JoinHandle<T>);

impl<T> Future for Background<T> {
	type Output = T;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
		// Nothing else aborts the work, so the only error is a panic, which goes on to the awaiting
		// task.
		Pin::new(&mut self.0).poll(cx).map(|result| result.unwrap_or_else(|error| resume_unwind(error.into_panic())))
	}
}

impl<T> Drop for Background<T> {
	fn drop(&mut self) {
		self.0.abort();
	}
}

/// Runs `future` on a shared Tokio runtime, which async clients such as `reqwest` and
/// `tokio-tungstenite` need.
pub fn background<F>(future: F) -> Background<F::Output>
where
	F: Future + Send + 'static,
	F::Output: Send + 'static,
{
	Background(RUNTIME.spawn(future))
}

/// Runs `work` on a thread meant for blocking calls, such as a blocking HTTP client's.
pub fn background_blocking<T, F>(work: F) -> Background<T>
where
	F: FnOnce() -> T + Send + 'static,
	T: Send + 'static,
{
	Background(RUNTIME.spawn_blocking(work))
}

/// Waits for `duration` without holding up the UI thread.
pub fn sleep(duration: Duration) -> Background<()> {
	// A Tokio timer can only be made on the runtime, so it's made inside the background future.
	background(async move { time::sleep(duration).await })
}
