// Generated — WebviewApp for "app"
// Route prefix: /app/
// Usage: builder.route_with("/app/", webview_app(), generated::app::AppAssets::build());

use foundation_macros::EmbedDirectoryAs;
use foundation_platform::WebviewApp;

#[derive(EmbedDirectoryAs)]
#[source = "public/app"]
pub struct AppAssets;

impl AppAssets {
    pub fn build() -> WebviewApp<AppAssets> { WebviewApp::new(AppAssets {}) }
}
