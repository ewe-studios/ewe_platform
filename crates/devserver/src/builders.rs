use tokio::sync::broadcast;

use crate::{
    assets,
    types::{JoinHandle, Result},
    BinaryApp, CargoShellBuilder, DirectoryWatcher, Operator, ParrellelOps, ProjectDefinition,
    StreamTCPApp,
};
use std::{sync, time};

pub struct HttpDevService {
    pub project: ProjectDefinition,
    pub package_changes: broadcast::Sender<()>,
    pub package_built: broadcast::Sender<()>,
    pub package_started: broadcast::Sender<()>,
}

// -- Constructors

impl HttpDevService {
    pub fn new(project: ProjectDefinition) -> Self {
        let (package_changes, _) = broadcast::channel::<()>(2);
        let (package_started, _) = broadcast::channel::<()>(2);
        let (package_built, _) = broadcast::channel::<()>(2);

        Self { project, package_changes, package_built, package_started }
    }
}

// -- Getters

// -- Core Starter

impl HttpDevService {
    pub async fn start(&mut self, canceller: broadcast::Receiver<()>) -> Result<JoinHandle<()>> {
        let package_started = &self.package_started;
        self.project.and_proxy_routes(move |routes| {
            // add the script for sse based refresh
            routes
                .entry(assets::RELOADER_SCRIPT_ENDPOINT.to_string())
                .or_insert(sync::Arc::new(assets::sse_endpoint_script));

            // sse endpoint that the script must call into
            routes
                .entry(assets::RELOADER_SSE_ENDPOINT.to_string())
                .or_insert(assets::create_sse_endpoint_handler(package_started.clone()));
        });

        let project_directory_watcher = DirectoryWatcher::new(
            self.project.watch_directories.clone(),
            self.package_changes.clone(),
        );

        // these two should be restartable
        // app_builder restarts when the file watcher says stuff changes
        let app_builder = CargoShellBuilder::shared(
            self.project.clone(),
            self.package_built.clone(),
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
            Box::new(project_directory_watcher),
        ]);

        let command_op = command.run(canceller);

        self.package_changes
            .send(())
            .expect("should have delivered trigger message");

        Ok(command_op)
    }
}
