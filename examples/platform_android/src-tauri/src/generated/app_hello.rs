// Generated — WebviewApp for "app-hello"
// Route prefix: /app-hello/
// Usage: builder.route_with("/app-hello/", webview_app(), generated::app_hello::AppAssets::build());

use foundation_macros::EmbedDirectoryAs;
use foundation_platform::WebviewApp;

#[derive(EmbedDirectoryAs)]
#[source = "public/app-hello"]
pub struct AppAssets;

impl AppAssets {
    pub fn build() -> WebviewApp<AppAssets> { WebviewApp::new(AppAssets {}) }
}
