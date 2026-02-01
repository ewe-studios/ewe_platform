#![allow(clippy::missing_panics_doc)]
#![allow(clippy::too_many_lines)]

use tokio::sync::broadcast;

use crate::{
    assets,
    types::{JoinHandle, Result},
    BinaryApp, CargoShellBuilder, DirectoryWatcher, FileChange, Operator, ParrellelOps,
    ProjectDefinition, StreamTCPApp,
};
use std::{sync, time};

pub struct HttpDevService {
    pub project: ProjectDefinition,
    pub package_built: broadcast::Sender<()>,
    pub package_started: broadcast::Sender<()>,
    pub trigger_rerun: broadcast::Sender<()>,
    pub page_reload: broadcast::Sender<FileChange>,
    pub package_changes: broadcast::Sender<FileChange>,
}

// -- Constructors

impl HttpDevService {
    #[must_use]
    pub fn new(project: ProjectDefinition) -> Self {
        let (trigger_rerun, _) = broadcast::channel::<()>(2);
        let (package_started, _) = broadcast::channel::<()>(2);
        let (package_built, _) = broadcast::channel::<()>(2);
        let (page_reload, _) = broadcast::channel::<FileChange>(2);
        let (package_changes, _) = broadcast::channel::<FileChange>(2);

        Self {
            project,
            package_built,
            package_started,
            trigger_rerun,
            page_reload,
            package_changes,
        }
    }
}

// -- Getters

// -- Core Starter

impl HttpDevService {
    pub async fn start(&mut self, canceller: broadcast::Receiver<()>) -> Result<JoinHandle<()>> {
        let page_reload_ref = &self.page_reload;
        self.project.and_proxy_routes(move |routes| {
            // add the script for sse based refresh
            routes
                .entry(assets::RELOADER_SCRIPT_ENDPOINT.to_string())
                .or_insert(sync::Arc::new(assets::sse_endpoint_script));

            // sse endpoint that the script must call into
            routes
                .entry(assets::RELOADER_SSE_ENDPOINT.to_string())
                .or_insert(assets::create_sse_endpoint_handler(page_reload_ref.clone()));
        });

        let project_builder_watcher = DirectoryWatcher::new(
            self.project.build_directories.clone(),
            self.package_changes.clone(),
        );

        let project_reloader_watcher = DirectoryWatcher::new(
            self.project.reload_directories.clone(),
            self.page_reload.clone(),
        );

        // these two should be restart-able
        // app_builder restarts when the file watcher says stuff changes
        let app_builder = CargoShellBuilder::shared(
            self.project.skip_rust_checks,
            self.project.stop_on_failure,
            self.project.clone(),
            self.package_built.clone(),
            self.trigger_rerun.clone(),
            self.package_changes.clone(),
        );

        // app_runner restarts when app_builder says its done building
        let app_runner = BinaryApp::shared(
            self.project.clone(),
            self.package_built.clone(),
            self.package_started.clone(),
        );

        let service_proxy =
            StreamTCPApp::shared(time::Duration::from_secs(1), self.project.proxy.clone());

        let command = ParrellelOps::new(vec![
            Box::new(app_builder),
            Box::new(app_runner),
            Box::new(service_proxy),
            Box::new(project_builder_watcher),
            Box::new(project_reloader_watcher),
        ]);

        let command_op = command.run(canceller);

        self.trigger_rerun
            .send(())
            .expect("should have delivered trigger message");

        Ok(command_op)
    }
}
