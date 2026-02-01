use anyhow::anyhow;
use ewe_templates::{minijinja, tinytemplate::TinyTemplate};
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{self, PathBuf},
    result,
    str::FromStr,
    sync::Arc,
};

type FileResult<T> = result::Result<T, anyhow::Error>;

pub enum FileContent<'a> {
    Text(String),
    Tiny(String, TinyTemplate<'a>),
    Jinja(String, Arc<minijinja::Environment<'a>>),
}

impl FileContent<'_> {
    pub fn run<S: Serialize>(&self, dest: path::PathBuf, value: Option<S>) -> FileResult<()> {
        match self {
            FileContent::Text(content) => {
                let mut file = fs::File::create(dest.as_path())?;
                let written = file.write(content.as_bytes())?;
                if written != content.len() {
                    return Err(anyhow!("written content does not match provided data size"));
                }
                Ok(())
            }
            FileContent::Jinja(name, templater) => {
                let mut file = fs::File::create(dest.as_path())?;

                let rendered = templater
                    .get_template(name.as_str())
                    .unwrap()
                    .render(value)?;

                let written = file.write(rendered.as_bytes())?;
                if written != rendered.len() {
                    return Err(anyhow!("written content does not match provided data size"));
                }
                Ok(())
            }
            FileContent::Tiny(name, templater) => {
                let mut file = fs::File::create(dest.as_path())?;

                let rendered = templater.render(name.as_str(), &value)?;
                let written = file.write(rendered.as_bytes())?;
                if written != rendered.len() {
                    return Err(anyhow!("written content does not match provided data size"));
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
                    ewe_trace::info!("Creating directory: {:?}", dir);
                }

                let mut builder = fs::DirBuilder::new();
                builder.recursive(true).create(dir.clone())?;

                for sub_command in commands {
                    sub_command.exec(dir.clone(), value.clone())?;
                }

                Ok(())
            }
            FileSystemCommand::Dir(dir, commands) => {
                let mut target_path = dest.clone();
                target_path.push(dir);

                if !target_path.exists() {
                    ewe_trace::info!("Creating directory: {:?}", target_path);
                }

                let mut builder = fs::DirBuilder::new();
                builder.recursive(true).create(target_path.clone())?;

                for sub_command in commands {
                    sub_command.exec(target_path.clone(), value.clone())?;
                }

                Ok(())
            }
            FileSystemCommand::File(file_name, content) => {
                let mut target_path = dest.clone();
                target_path.push(file_name);

                ewe_trace::info!("Creating file: {:?}", target_path);

                content.run(target_path, Some(value))?;

                Ok(())
            }
            FileSystemCommand::FilePath(file_name, content) => {
                ewe_trace::info!("Creating file: {:?}", file_name);
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

#[cfg(test)]
mod tests {
    use super::{FileContent, FileResult, FileSystemCommand, Templater};
    use ewe_templates::{minijinja, tinytemplate::TinyTemplate};
    use rand::Rng;
    use serde_json::{json, Value};
    use std::{env, fs, io::Read, path, sync};

    fn random_directory_name(prefix: &str) -> String {
        let suffix: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        format!("{}_{}", prefix, suffix)
    }

    fn clean_up_directory(target: path::PathBuf) {
        fs::remove_dir_all(target).expect("should have deleted directory");
    }

    fn create_jinja_template() -> FileResult<minijinja::Environment<'static>> {
        let mut tt = minijinja::Environment::new();

        tt.add_template("world", "{{country}} wonderworld!")?;
        tt.add_template("hello", "Welcome to hello {{name}}!")?;

        tt.add_template("index", r"{% include 'hello' %} {% include 'world' %}")?;

        Ok(tt)
    }

    fn create_tiny_template() -> FileResult<TinyTemplate<'static>> {
        let mut tt = TinyTemplate::new();

        tt.add_template("world", "{country} wonderworld!")?;
        tt.add_template("hello", "Welcome to hello {name}!")?;

        tt.add_template(
            "index",
            r"{{ call hello with @root }} {{ call world with @root }}",
        )?;

        Ok(tt)
    }

    #[test]
    fn test_can_create_directory() {
        let tmp_dir = env::temp_dir();

        let mut target = tmp_dir.clone();
        target.push("temple");

        let mut tml = Templater::new(target.to_str().unwrap());
        tml.add(FileSystemCommand::Dir(String::from("weeds"), vec![]));

        let data: Value = json!({
            "code": 200,
            "name": "Alex",
            "country": "Nigeria",
        });

        assert!(matches!(tml.run(&data), FileResult::Ok(())));

        let mut expected_path = target.clone();
        expected_path.push("weeds");

        assert!(expected_path.exists());
        assert!(expected_path.is_dir());

        clean_up_directory(target);
    }

    #[test]
    fn test_can_create_directory_with_file_with_jinja() {
        let tmp_dir = env::temp_dir();

        let mut target = tmp_dir.clone();
        target.push(random_directory_name("temple"));

        let mut tml = Templater::new(target.to_str().unwrap());

        let templ = sync::Arc::new(create_jinja_template().expect("created sample template"));

        tml.add(FileSystemCommand::Dir(
            String::from("weeds"),
            vec![FileSystemCommand::File(
                String::from("index.md"),
                FileContent::Jinja(String::from("index"), templ),
            )],
        ));

        let data: Value = json!({
            "code": 200,
            "name": "Alex",
            "country": "Nigeria",
        });

        tml.run(&data).expect("finished executing template");

        let mut expected_path = target.clone();
        expected_path.push("weeds");

        assert!(expected_path.exists());
        assert!(expected_path.is_dir());

        let mut expected_file = target.clone();
        expected_file.push("weeds");
        expected_file.push("index.md");

        assert!(expected_file.exists());
        assert!(expected_file.is_file());

        let mut created_file = fs::File::open(expected_file).expect("should read created file");

        let mut content = String::new();
        let read = created_file
            .read_to_string(&mut content)
            .expect("should read to string");

        assert_ne!(read, 0);

        assert_eq!(content, "Welcome to hello Alex! Nigeria wonderworld!");

        clean_up_directory(target);
    }

    #[test]
    fn test_can_create_directory_with_file_with_tiny() {
        let tmp_dir = env::temp_dir();

        let mut target = tmp_dir.clone();
        target.push(random_directory_name("temple"));

        let mut tml = Templater::new(target.to_str().unwrap());

        let templ = create_tiny_template().expect("created sample template");

        tml.add(FileSystemCommand::Dir(
            String::from("weeds"),
            vec![FileSystemCommand::File(
                String::from("index.md"),
                FileContent::Tiny(String::from("index"), templ),
            )],
        ));

        let data: Value = json!({
            "code": 200,
            "name": "Alex",
            "country": "Nigeria",
        });

        tml.run(&data).expect("finished executing template");

        let mut expected_path = target.clone();
        expected_path.push("weeds");

        assert!(expected_path.exists());
        assert!(expected_path.is_dir());

        let mut expected_file = target.clone();
        expected_file.push("weeds");
        expected_file.push("index.md");

        assert!(expected_file.exists());
        assert!(expected_file.is_file());

        let mut created_file = fs::File::open(expected_file).expect("should read created file");

        let mut content = String::new();
        let read = created_file
            .read_to_string(&mut content)
            .expect("should read to string");

        assert_ne!(read, 0);

        assert_eq!(content, "Welcome to hello Alex! Nigeria wonderworld!");

        clean_up_directory(target);
    }
}
