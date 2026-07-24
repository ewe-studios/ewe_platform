//! Tauri plugin — bridges foundation_platform_native to Kotlin/Swift.
//!
//! Register via `PlatformBuilder::plugin(ewe_platform_native::native::plugin())`.
//! The plugin setup registers the Kotlin `EwePlatformPlugin` and stores the
//! `PluginHandle` in Tauri managed state so `ModalIpc` / `DialogIpc` etc.
//! can call `run_mobile_plugin`.

use tauri::plugin::{PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

/// Managed Tauri state — holds PluginHandles for each native capability.
pub struct EweNativeHandles {
    pub modal: PluginHandle<Wry>,
}

/// Build the Tauri plugin. Register this on the app via:
/// ```ignore
/// builder.plugin(ewe_platform_native::native::plugin::plugin());
/// ```
#[must_use]
pub fn plugin() -> TauriPlugin<Wry> {
    tauri::plugin::Builder::new("ewe-platform-native")
        .setup(|app, api| {
            // register_android_plugin is only available on mobile targets
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin(
                    "com.ewe.platform",
                    "EwePlatformPlugin",
                )?;
                app.manage(EweNativeHandles { modal: handle });
            }
            let _ = (app, api);
            Ok(())
        })
        .build()
}
