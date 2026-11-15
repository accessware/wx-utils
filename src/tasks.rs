//! Async tasks that run on the UI thread, and the work they hand to background threads.

use std::{
	cell::RefCell,
	collections::{HashMap, hash_map::Entry},
	panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
	pin::Pin,
	sync::{LazyLock, mpsc},
	task::{Context, Poll},
	thread,
	time::Duration,
};

use async_task::Runnable;
pub use async_task::Task;
use tokio::{
	runtime::{Builder, Runtime},
	sync::oneshot,
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
pub struct Background<T>(Work<T>);

enum Work<T> {
	Spawned(JoinHandle<T>),
	/// Queued work hands its output straight to the awaiting task, so tasks waiting on one queue
	/// wake in the order their work finished.
	Queued(oneshot::Receiver<thread::Result<T>>),
}

impl<T> Future for Background<T> {
	type Output = T;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
		match &mut self.0 {
			// Nothing else aborts the work, so the only error is a panic, which goes on to the
			// awaiting task.
			Work::Spawned(handle) => {
				Pin::new(handle).poll(cx).map(|result| result.unwrap_or_else(|error| resume_unwind(error.into_panic())))
			}
			Work::Queued(receiver) => Pin::new(receiver).poll(cx).map(|result| match result {
				Ok(Ok(output)) => output,
				Ok(Err(panic)) => resume_unwind(panic),
				Err(_) => unreachable!("a queued job always sends its output"),
			}),
		}
	}
}

impl<T> Drop for Background<T> {
	fn drop(&mut self) {
		if let Work::Spawned(handle) = &self.0 {
			handle.abort();
		}
	}
}

/// Runs `future` on a shared Tokio runtime, which async clients such as `reqwest` and
/// `tokio-tungstenite` need.
pub fn background<F>(future: F) -> Background<F::Output>
where
	F: Future + Send + 'static,
	F::Output: Send + 'static,
{
	Background(Work::Spawned(RUNTIME.spawn(future)))
}

/// Runs `work` on a thread meant for blocking calls, such as a blocking HTTP client's.
pub fn background_blocking<T, F>(work: F) -> Background<T>
where
	F: FnOnce() -> T + Send + 'static,
	T: Send + 'static,
{
	Background(Work::Spawned(RUNTIME.spawn_blocking(work)))
}

/// Waits for `duration` without holding up the UI thread.
pub fn sleep(duration: Duration) -> Background<()> {
	// A Tokio timer can only be made on the runtime, so it's made inside the background future.
	background(async move { time::sleep(duration).await })
}

type Job = Box<dyn FnOnce() + Send>;

/// Runs blocking work on a background thread of its own, one job at a time and in the order it
/// was queued, for calls whose order matters, such as a favorite and the unfavorite after it.
///
/// The thread ends once every clone of the queue is dropped and the jobs already queued are done.
#[derive(Clone)]
pub struct Queue(mpsc::Sender<Job>);

impl Queue {
	/// # Panics
	///
	/// If the queue's thread can't be started, since the queue would otherwise never run anything.
	#[must_use]
	pub fn new() -> Self {
		let (jobs, queued) = mpsc::channel::<Job>();
		thread::Builder::new()
			.name("wx-utils-queue".into())
			.spawn(move || {
				for job in queued {
					// A panic stops only its own job: the hook has reported it, and the jobs behind it
					// would otherwise never run.
					let _ = catch_unwind(AssertUnwindSafe(job));
				}
			})
			.expect("the queue's thread should start");
		Self(jobs)
	}

	/// Queues `work` without waiting for it, for work that reports back some other way.
	pub fn push(&self, work: impl FnOnce() + Send + 'static) {
		// The thread only stops once every sender is gone, and this queue holds one.
		let _ = self.0.send(Box::new(work));
	}

	/// Queues `work`, and gives back its output once it has run.
	pub fn run<T, F>(&self, work: F) -> Background<T>
	where
		F: FnOnce() -> T + Send + 'static,
		T: Send + 'static,
	{
		let (output, receiver) = oneshot::channel();
		// Caught here as well, so the panic reaches the awaiting task.
		self.push(move || {
			let _ = output.send(catch_unwind(AssertUnwindSafe(work)));
		});
		Background(Work::Queued(receiver))
	}
}

impl Default for Queue {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use std::{
		panic::{AssertUnwindSafe, catch_unwind},
		sync::{Arc, Mutex},
		thread,
		time::Duration,
	};

	use super::{Queue, RUNTIME};

	#[test]
	fn queue_runs_jobs_in_order() {
		let queue = Queue::new();
		let ran = Arc::new(Mutex::new(Vec::new()));
		let outputs: Vec<_> = (0..20_u64)
			.map(|n| {
				let ran = ran.clone();
				queue.run(move || {
					// Later jobs are quicker, so they'd overtake earlier ones if anything ran them at once.
					thread::sleep(Duration::from_millis(20 - n));
					ran.lock().unwrap().push(n);
					n * 2
				})
			})
			.collect();
		for (n, output) in (0..).zip(outputs) {
			assert_eq!(RUNTIME.block_on(output), n * 2);
		}
		assert_eq!(*ran.lock().unwrap(), (0..20).collect::<Vec<_>>());
	}

	#[test]
	fn queue_goes_on_after_a_panic() {
		let queue = Queue::new();
		queue.push(|| panic!("the pushed job failed"));
		let panicked = queue.run(|| panic!("the job failed"));
		let next = queue.run(|| 7);
		assert!(catch_unwind(AssertUnwindSafe(|| RUNTIME.block_on(panicked))).is_err());
		assert_eq!(RUNTIME.block_on(next), 7);
	}
}
