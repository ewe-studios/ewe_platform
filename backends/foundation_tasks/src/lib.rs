//! Register typed tasks and dispatch them from the command line.
use clap::{Arg, ArgMatches, Args, Command};
use foundation_context::Ctx;
use futures_util::future::BoxFuture;
use std::future::Future;
use std::{collections::BTreeMap, ffi::OsString, sync::Arc};

mod process;
pub use process::*;
pub mod build;
#[cfg(feature = "install")]
pub mod install;
#[cfg(feature = "scripts")]
pub mod scripts;
pub use foundation_macros::{task_entry as task, tasks};

pub type TaskResult = Result<(), TaskError>;

#[derive(Debug, derive_more::Display, derive_more::Error)]
pub enum TaskError {
    #[display("Invalid task name or path: {_0}")]
    InvalidName(#[error(not(source))] String),
    #[display("Task {name} from {second} conflicts with {first}")]
    Conflict {
        name: String,
        first: &'static str,
        second: &'static str,
    },
    #[display("Missing service {_0}")]
    MissingService(#[error(not(source))] &'static str),
    #[display("{_0}")]
    Arguments(#[error(not(source))] clap::Error),
    #[display("{_0}")]
    Operation(#[error(source)] Box<dyn std::error::Error + Send + Sync>),
    #[display("{task}: {source}")]
    Task {
        task: String,
        #[error(source)]
        source: Box<TaskError>,
    },
    #[display("{program:?}: {source}")]
    Process {
        program: OsString,
        #[error(source)]
        source: std::io::Error,
    },
    #[display("{program:?} exited with status {code}")]
    Exit { program: OsString, code: i32 },
    #[display("{program:?} terminated by signal {signal}")]
    Signal { program: OsString, signal: i32 },
    #[display("Task cancelled")]
    Cancelled,
    #[display("{source}; cleanup failed: {cleanup}")]
    Cleanup {
        #[error(source)]
        source: Box<TaskError>,
        cleanup: Box<TaskError>,
    },
}

impl TaskError {
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Arguments(error) => u8::try_from(error.exit_code()).unwrap_or(2),
            Self::Exit { code, .. } => u8::try_from(*code).unwrap_or(1),
            Self::Signal { signal, .. } => signal
                .checked_add(128)
                .and_then(|n| u8::try_from(n).ok())
                .unwrap_or(1),
            Self::Cancelled => 130,
            Self::Task { source, .. } | Self::Cleanup { source, .. } => source.exit_code(),
            _ => 1,
        }
    }

    pub fn operation(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Operation(error.into())
    }
}

#[derive(Clone)]
pub struct TaskContext {
    root: Ctx,
    cancellation: Cancellation,
}

impl TaskContext {
    /// Return the context shared with the executable host.
    #[must_use]
    pub fn context(&self) -> &Ctx {
        &self.root
    }

    /// # Errors
    /// Returns a typed error if the host does not install the requested service.
    pub fn require<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, TaskError> {
        self.root
            .get::<T>()
            .ok_or(TaskError::MissingService(std::any::type_name::<T>()))
    }

    #[must_use]
    pub fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }

    /// # Errors
    /// Returns a start, stream, cancellation, cleanup, or child exit error.
    pub async fn exec(&self, command: ProcessCommand) -> TaskResult {
        let env = self.require::<TaskEnv>()?;
        env.process
            .run(command, env.output.clone(), self.cancellation.clone())
            .await
    }

    /// # Errors
    /// Returns the same errors as `exec` and uses the supplied output sink.
    pub async fn exec_with_output(
        &self,
        command: ProcessCommand,
        output: Arc<dyn TaskOutput>,
    ) -> TaskResult {
        let env = self.require::<TaskEnv>()?;
        env.process
            .run(command, output, self.cancellation.clone())
            .await
    }
}

type Handler = dyn Fn(TaskContext, ArgMatches) -> BoxFuture<'static, TaskResult> + Send + Sync;

struct Definition {
    command: Command,
    summary: String,
    handler: Box<Handler>,
}

pub struct TaskSet {
    owner: &'static str,
    namespace: Option<String>,
    tasks: BTreeMap<String, Definition>,
    aliases: Vec<(&'static str, Vec<&'static str>)>,
    groups: BTreeMap<&'static str, String>,
}

impl TaskSet {
    #[must_use]
    pub fn new(owner: &'static str) -> Self {
        Self {
            owner,
            namespace: None,
            tasks: BTreeMap::new(),
            aliases: Vec::new(),
            groups: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// # Errors
    /// Rejects invalid or duplicate names.
    pub fn add<A, F, Fut>(
        &mut self,
        name: &'static str,
        summary: &'static str,
        handler: F,
    ) -> TaskResult
    where
        A: Args + Send + 'static,
        F: Fn(TaskContext, A) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = TaskResult> + Send + 'static,
    {
        let command = A::augment_args(Command::new(name).about(summary));
        self.add_command(name, summary, command, move |ctx, matches| {
            let args = A::from_arg_matches(&matches);
            let future = args.map(|args| handler(ctx, args));
            async { future.map_err(TaskError::Arguments)?.await }
        })
    }

    /// Adapt an existing command definition and its argument parser.
    ///
    /// # Errors
    /// Rejects invalid or duplicate names.
    pub fn add_command<F, Fut>(
        &mut self,
        name: &str,
        summary: impl Into<String>,
        command: Command,
        handler: F,
    ) -> TaskResult
    where
        F: Fn(TaskContext, ArgMatches) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = TaskResult> + Send + 'static,
    {
        validate_name(name)?;
        if self.tasks.contains_key(name) {
            return Err(conflict(name, self.owner, self.owner));
        }
        self.tasks.insert(
            name.to_owned(),
            Definition {
                command,
                summary: summary.into(),
                handler: Box::new(move |ctx, matches| Box::pin(handler(ctx, matches))),
            },
        );
        Ok(())
    }

    /// # Errors
    /// Rejects unknown names and reserved or invalid command paths.
    pub fn expose(&mut self, name: &'static str, path: &[&'static str]) -> TaskResult {
        if path.is_empty() || !path.iter().all(|segment| direct_segment(segment)) {
            return Err(TaskError::InvalidName(path.join(" ")));
        }
        if matches!(path[0], "task" | "help") {
            return Err(conflict(&path.join(" "), "task runner", self.owner));
        }
        if !self.tasks.contains_key(name) {
            return Err(TaskError::InvalidName(name.into()));
        }
        self.aliases.push((name, path.to_vec()));
        Ok(())
    }

    pub fn describe_group(&mut self, group: &'static str, summary: impl Into<String>) {
        self.groups.insert(group, summary.into());
    }
}

fn direct_segment(segment: &str) -> bool {
    segment
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_lowercase)
        && segment.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn validate_name(name: &str) -> TaskResult {
    if name.split(':').all(|segment| {
        direct_segment(segment)
            && !segment.contains('_')
            && !segment.ends_with('-')
            && !segment.contains("--")
    }) {
        Ok(())
    } else {
        Err(TaskError::InvalidName(name.into()))
    }
}

fn conflict(name: &str, first: &'static str, second: &'static str) -> TaskError {
    TaskError::Conflict {
        name: name.into(),
        first,
        second,
    }
}

pub struct TaskRunner {
    root: Ctx,
    argv: Vec<OsString>,
    label: &'static str,
    about: &'static str,
    tasks: BTreeMap<String, (&'static str, Definition)>,
    aliases: BTreeMap<Vec<&'static str>, (&'static str, String)>,
    namespaces: BTreeMap<String, &'static str>,
    groups: BTreeMap<&'static str, String>,
}

impl TaskRunner {
    #[must_use]
    pub fn new(root: Ctx, argv: Vec<OsString>) -> Self {
        Self {
            root,
            argv,
            label: "tasks",
            about: "Run available tasks",
            tasks: BTreeMap::new(),
            aliases: BTreeMap::new(),
            namespaces: BTreeMap::new(),
            groups: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_name(mut self, label: &'static str, about: &'static str) -> Self {
        self.label = label;
        self.about = about;
        self
    }

    #[must_use]
    pub fn context(&self) -> &Ctx {
        &self.root
    }

    /// # Errors
    /// Rejects invalid names and any conflict with earlier registrations.
    pub fn register(&mut self, set: TaskSet) -> TaskResult {
        if let Some(namespace) = set.namespace.as_deref() {
            validate_name(namespace)?;
            if let Some(&owner) = self.namespaces.get(namespace) {
                if owner != set.owner {
                    return Err(conflict(namespace, owner, set.owner));
                }
            }
        }
        let mut qualified = BTreeMap::new();
        for name in set.tasks.keys() {
            let full = set
                .namespace
                .as_deref()
                .map_or_else(|| name.clone(), |ns| format!("{ns}:{name}"));
            if let Some((owner, _)) = self.tasks.get(full.as_str()) {
                return Err(conflict(&full, owner, set.owner));
            }
            qualified.insert(name.clone(), full);
        }
        let mut pending_paths = BTreeMap::new();
        for (name, path) in set.aliases {
            if let Some((_, &(owner, _))) = self
                .aliases
                .iter()
                .chain(&pending_paths)
                .find(|(prior, _)| prior.starts_with(&path) || path.starts_with(prior))
            {
                return Err(conflict(&path.join(" "), owner, set.owner));
            }
            pending_paths.insert(path, (set.owner, qualified[name].clone()));
        }
        for (name, definition) in set.tasks {
            self.tasks
                .insert(qualified[&name].clone(), (set.owner, definition));
        }
        for (path, (owner, name)) in pending_paths {
            self.aliases.insert(path, (owner, name));
        }
        if let Some(namespace) = set.namespace {
            self.namespaces.insert(namespace, set.owner);
        }
        self.groups.extend(set.groups);
        Ok(())
    }

    /// Let this executable replace the default help command with a registered guide.
    ///
    /// # Errors
    /// Returns an error when the named task does not exist.
    pub fn use_help_task(&mut self, name: &str) -> TaskResult {
        let (owner, _) = self
            .tasks
            .get(name)
            .ok_or_else(|| TaskError::InvalidName(name.into()))?;
        self.aliases.insert(vec!["help"], (*owner, name.to_owned()));
        Ok(())
    }

    /// Return the command tree used by the dispatcher and help.
    pub fn root_command(&self) -> Command {
        let mut run = Command::new("run")
            .about("Run a named task")
            .subcommand_required(true);
        for (name, (_, definition)) in &self.tasks {
            run = run.subcommand(definition.command.clone().name(name.clone()));
        }
        let task = Command::new("task")
            .about("List tasks or run a named task")
            .subcommand_required(true)
            .disable_help_subcommand(true)
            .subcommand(
                Command::new("ls")
                    .visible_alias("list")
                    .about("List available tasks"),
            )
            .subcommand(
                Command::new("help")
                    .about("Show task help")
                    .arg(Arg::new("name")),
            )
            .subcommand(run);
        let mut root = Command::new(self.label)
            .about(self.about)
            .disable_help_subcommand(self.aliases.contains_key(&vec!["help"]))
            .subcommand_required(true)
            .subcommand(task);
        let mut paths = CommandPaths::default();
        for (path, (_, name)) in &self.aliases {
            paths.insert(path, name);
        }
        for command in paths.commands(self) {
            root = root.subcommand(command);
        }
        root
    }

    async fn write_help(&self, error: clap::Error) -> TaskResult {
        if error.use_stderr() {
            return Err(TaskError::Arguments(error));
        }
        self.output(error.to_string().as_bytes()).await
    }

    async fn output(&self, bytes: &[u8]) -> TaskResult {
        self.root
            .get::<TaskEnv>()
            .ok_or(TaskError::MissingService(std::any::type_name::<TaskEnv>()))?
            .output
            .write(OutputStream::Stdout, bytes)
            .await
            .map_err(TaskError::operation)
    }

    /// # Errors
    /// Returns an argument, service, or selected task error.
    ///
    /// # Panics
    /// Panics if the argument parser returns a result without the required command.
    pub async fn run(self) -> TaskResult {
        let command = self.root_command();
        let matches = match command.clone().try_get_matches_from(&self.argv) {
            Ok(matches) => matches,
            Err(error) => return self.write_help(error).await,
        };
        let (mut segment, mut arguments) =
            matches.subcommand().expect("top-level command required");
        let name = if segment == "task" {
            let (subcommand, matches) = arguments.subcommand().expect("task subcommand required");
            match subcommand {
                "ls" => {
                    let width = self.tasks.keys().map(String::len).max().unwrap_or(0);
                    for (name, (_, entry)) in &self.tasks {
                        self.output(format!("{name:width$}  {}\n", entry.summary).as_bytes())
                            .await?;
                    }
                    return Ok(());
                }
                "help" => {
                    let mut task = command.find_subcommand("task").expect("task group").clone();
                    let mut help = if let Some(name) = matches.get_one::<String>("name") {
                        self.tasks
                            .get(name.as_str())
                            .map(|(_, entry)| {
                                entry
                                    .command
                                    .clone()
                                    .bin_name(format!("{} task run {name}", self.label))
                            })
                            .ok_or_else(|| {
                                TaskError::Arguments(task.error(
                                    clap::error::ErrorKind::InvalidSubcommand,
                                    format!("Unknown task {name:?}"),
                                ))
                            })?
                    } else {
                        task.bin_name(format!("{} task", self.label))
                    };
                    return self
                        .output(help.render_long_help().to_string().as_bytes())
                        .await;
                }
                "run" => {
                    (segment, arguments) = matches.subcommand().expect("named task required");
                    segment.to_owned()
                }
                _ => unreachable!(),
            }
        } else {
            let mut path = vec![segment];
            while !self
                .aliases
                .keys()
                .any(|alias| alias.as_slice() == path.as_slice())
            {
                let (next, nested) = arguments.subcommand().expect("direct leaf required");
                path.push(next);
                arguments = nested;
            }
            self.aliases
                .iter()
                .find(|(alias, _)| alias.as_slice() == path.as_slice())
                .expect("known direct path")
                .1
                 .1
                .clone()
        };
        let cancellation = self
            .root
            .get::<TaskEnv>()
            .map_or_else(Cancellation::new, |env| env.cancellation.clone());
        let ctx = TaskContext {
            root: self.root,
            cancellation,
        };
        (self.tasks[&name].1.handler)(ctx, arguments.clone())
            .await
            .map_err(|source| TaskError::Task {
                task: name,
                source: Box::new(source),
            })
    }
}

#[derive(Default)]
struct CommandPaths {
    leaf: Option<String>,
    children: BTreeMap<&'static str, Self>,
}

impl CommandPaths {
    fn insert(&mut self, path: &[&'static str], name: &str) {
        if let Some((&segment, tail)) = path.split_first() {
            self.children.entry(segment).or_default().insert(tail, name);
        } else {
            self.leaf = Some(name.to_owned());
        }
    }

    fn commands(&self, runner: &TaskRunner) -> Vec<Command> {
        self.children
            .iter()
            .map(|(&segment, node)| {
                if let Some(name) = &node.leaf {
                    runner.tasks[name].1.command.clone().name(segment)
                } else {
                    let mut command = Command::new(segment)
                        .subcommand_required(true)
                        .subcommands(node.commands(runner));
                    if let Some(summary) = runner.groups.get(segment) {
                        command = command.about(summary.clone());
                    }
                    command
                }
            })
            .collect()
    }
}
