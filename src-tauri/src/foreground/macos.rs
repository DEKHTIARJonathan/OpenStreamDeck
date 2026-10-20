use super::*;
use block2::RcBlock;
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSWorkspace, NSWorkspaceDidActivateApplicationNotification};
use objc2_foundation::{NSBundle, NSDate, NSNotification, NSRunLoop, NSString};
use std::ptr::NonNull;
use std::time::{Duration, Instant};

pub(super) fn identity(path: &str) -> Result<String, String> {
    autoreleasepool(|_| {
        NSBundle::bundleWithPath(&NSString::from_str(path))
            .and_then(|bundle| bundle.bundleIdentifier())
            .map(|identity| identity.to_string())
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| "Selected application has no bundle identifier".into())
    })
}
fn current(workspace: &NSWorkspace) -> Observation {
    let Some(app) = workspace.frontmostApplication() else {
        return Ok(None);
    };
    let identity = app
        .bundleIdentifier()
        .ok_or("Foreground application has no bundle identifier")?
        .to_string();
    let name = app
        .localizedName()
        .map(|name| name.to_string())
        .unwrap_or_else(|| identity.clone());
    Ok(Some(ForegroundApplication {
        name,
        identity: ApplicationIdentity {
            platform: "macos".into(),
            value: identity,
        },
    }))
}
pub(super) fn observe(stop: Arc<AtomicBool>, callback: Callback) {
    // NSWorkspace's process-wide notification delivery also needs the main
    // application run loop, owned by Tauri. Standalone diagnostic harnesses
    // must pump that loop while waiting; this worker's loop is not a substitute.
    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let center = workspace.notificationCenter();
        let dirty = Arc::new(AtomicBool::new(true));
        let notification_dirty = dirty.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            notification_dirty.store(true, Ordering::Release);
        });
        // Registration precedes the initial query. Notification callbacks only
        // invalidate the latest snapshot; delayed callbacks cannot restore old apps.
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceDidActivateApplicationNotification),
                None,
                None,
                &block,
            )
        };
        let run_loop = NSRunLoop::currentRunLoop();
        let origin = Instant::now();
        let mut schedule = QuerySchedule::default();
        while !stop.load(Ordering::Acquire) {
            if schedule.due(
                dirty.swap(false, Ordering::AcqRel),
                origin.elapsed().as_millis() as u64,
            ) {
                let result = autoreleasepool(|_| current(&workspace));
                schedule.observed(&result, origin.elapsed().as_millis() as u64);
                callback(result);
            }
            autoreleasepool(|_| {
                run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.02));
            });
            // A run loop without input sources can return immediately.
            std::thread::sleep(Duration::from_millis(5));
        }
        unsafe {
            center.removeObserver((*observer).as_ref());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundle_identity_comes_from_native_metadata_not_its_display_name() {
        let directory = tempfile::tempdir().unwrap();
        let bundle = directory.path().join("Different display name.app");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents/Info.plist"), r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.openstreamdeck.fixture</string><key>CFBundleName</key><string>Fixture application</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#).unwrap();
        assert_eq!(
            identity(bundle.to_str().unwrap()).unwrap(),
            "org.openstreamdeck.fixture"
        );
        assert!(identity(directory.path().to_str().unwrap()).is_err());
    }
}
