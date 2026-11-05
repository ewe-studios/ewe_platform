use crate::tinytemplate::TinyTemplate;
use derive_more::{Display, Error};
use foundation_errstacks::{ErrorTrace, IntoErrorTrace};
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{self, PathBuf},
    result,
    str::FromStr,
    sync::Arc,
};

#[derive(Debug, Display, Error)]
#[display("{_0}")]
#[error(ignore)]
pub struct RenderError(String);

#[derive(Debug, Display, Error)]
pub enum PackagingError {
    #[display("IO error: {_0}")]
    Io(std::io::Error),
    #[display("written content does not match provided data size")]
    ContentSizeMismatch,
    #[display("template render failed: {_0}")]
    TemplateRender(RenderError),
}

type FileResult<T> = result::Result<T, ErrorTrace<PackagingError>>;

fn io_err(e: std::io::Error) -> ErrorTrace<PackagingError> {
    PackagingError::Io(e).into_error_trace()
}

fn render_err(e: impl std::fmt::Display) -> ErrorTrace<PackagingError> {
    PackagingError::TemplateRender(RenderError(e.to_string())).into_error_trace()
}

pub enum FileContent<'a> {
    Text(String),
    Tiny(String, TinyTemplate<'a>),
    Jinja(String, Arc<minijinja::Environment<'a>>),
}

impl FileContent<'_> {
    pub fn run<S: Serialize>(&self, dest: path::PathBuf, value: Option<S>) -> FileResult<()> {
        match self {
            FileContent::Text(content) => {
                let mut file = fs::File::create(dest.as_path()).map_err(io_err)?;
                let written = file.write(content.as_bytes()).map_err(io_err)?;
                if written != content.len() {
                    return Err(PackagingError::ContentSizeMismatch.into_error_trace());
                }
                Ok(())
            }
            FileContent::Jinja(name, templater) => {
                let mut file = fs::File::create(dest.as_path()).map_err(io_err)?;

                let rendered = templater
                    .get_template(name.as_str())
                    .unwrap()
                    .render(value)
                    .map_err(render_err)?;

                let written = file.write(rendered.as_bytes()).map_err(io_err)?;
                if written != rendered.len() {
                    return Err(PackagingError::ContentSizeMismatch.into_error_trace());
                }
                Ok(())
            }
            FileContent::Tiny(name, templater) => {
                let mut file = fs::File::create(dest.as_path()).map_err(io_err)?;

                let rendered = templater
                    .render(name.as_str(), &value)
                    .map_err(render_err)?;
                let written = file.write(rendered.as_bytes()).map_err(io_err)?;
                if written != rendered.len() {
                    return Err(PackagingError::ContentSizeMismatch.into_error_trace());
                }
                Ok(())
            }
        }
    }
}

pub enum FileSystemCommand<'a> {
    Dir(String, Vec<FileSystemCommand<'a>>),
    DirPath(PathBuf, Vec<FileSystemCommand<'a>>),
    File(String, FileContent<'a>),
    FilePath(PathBuf, FileContent<'a>),
}

impl FileSystemCommand<'_> {
    fn exec<S: Serialize + Clone>(&self, dest: path::PathBuf, value: S) -> FileResult<()> {
        match self {
            FileSystemCommand::DirPath(dir, commands) => {
                if !dir.exists() {
                    tracing::info!("Creating directory: {:?}", dir);
                }

                let mut builder = fs::DirBuilder::new();
                builder
                    .recursive(true)
                    .create(dir.clone())
                    .map_err(io_err)?;

                for sub_command in commands {
                    sub_command.exec(dir.clone(), value.clone())?;
                }

                Ok(())
            }
            FileSystemCommand::Dir(dir, commands) => {
                let mut target_path = dest.clone();
                target_path.push(dir);

                if !target_path.exists() {
                    tracing::info!("Creating directory: {:?}", target_path);
                }

                let mut builder = fs::DirBuilder::new();
                builder
                    .recursive(true)
                    .create(target_path.clone())
                    .map_err(io_err)?;

                for sub_command in commands {
                    sub_command.exec(target_path.clone(), value.clone())?;
                }

                Ok(())
            }
            FileSystemCommand::File(file_name, content) => {
                let mut target_path = dest.clone();
                target_path.push(file_name);

                tracing::info!("Creating file: {:?}", target_path);

                content.run(target_path, Some(value))?;

                Ok(())
            }
            FileSystemCommand::FilePath(file_name, content) => {
                tracing::info!("Creating file: {:?}", file_name);
                content.run(file_name.clone(), Some(value))?;
                Ok(())
            }
        }
    }
}

pub struct Templater<'a> {
    dest: path::PathBuf,
    commands: Vec<FileSystemCommand<'a>>,
}

impl<'a> Templater<'a> {
    #[must_use]
    pub fn new(dest: &'a str) -> Self {
        Self {
            dest: path::PathBuf::from_str(dest).expect("created PathBuf from str"),
            commands: Vec::with_capacity(5),
        }
    }

    pub fn from<S>(dest: S) -> Self
    where
        S: Into<PathBuf>,
    {
        Self {
            dest: dest.into(),
            commands: Vec::with_capacity(5),
        }
    }

    pub fn add(&mut self, command: FileSystemCommand<'a>) {
        self.commands.push(command);
    }

    pub fn run<S: Serialize>(&mut self, value: S) -> FileResult<()> {
        for command in &self.commands {
            command.exec(self.dest.clone(), &value)?;
        }
        Ok(())
    }
}
