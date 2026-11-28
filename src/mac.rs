//! macOS integration that wxWidgets doesn't provide.
use std::path::Path;

use objc2::MainThreadMarker;
use objc2_app_kit::NSDocumentController;
use objc2_foundation::{NSString, NSURL};

/// Adds `path` to macOS's recent documents, which the Dock icon's menu and Apple menu > Recent Items list.
///
/// macOS lists only files of a type the app's `Info.plist` declares in `CFBundleDocumentTypes`.
pub fn note_recent_document(path: &Path) {
	let Some(mtm) = MainThreadMarker::new() else {
		tracing::warn!(path = %path.display(), "a recent document can only be noted on the main thread");
		return;
	};
	let Some(path) = path.to_str() else {
		tracing::warn!(path = %path.display(), "a recent document's path isn't UTF-8");
		return;
	};
	let url = NSURL::fileURLWithPath(&NSString::from_str(path));
	NSDocumentController::sharedDocumentController(mtm).noteNewRecentDocumentURL(&url);
}
