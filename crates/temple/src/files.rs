use anyhow::anyhow;
use serde_json::{json, Value};
use std::{fs, io::Write, path, result, str::FromStr};
use tinytemplate::TinyTemplate;

type FileResult<T> = result::Result<T, anyhow::Error>;

pub enum FileContent<'a> {
    Text(String),
    Template(String, TinyTemplate<'a>),
}

impl<'a> FileContent<'a> {
    pub fn exec(&self, dest: path::PathBuf, mut value: Option<&Value>) -> FileResult<()> {
        match self {
            FileContent::Text(content) => {
                let mut file = fs::File::create(dest.as_path())?;
                let written = file.write(content.as_bytes())?;
                if written != content.len() {
                    return Err(anyhow!("written content does not match provided data size"));
                }
                Ok(())
            }
            FileContent::Template(name, templater) => {
                let mut file = fs::File::create(dest.as_path())?;

                let mut context = &json!({});
                if let Some(v) = value.take() {
                    context = v;
                }

                let rendered = templater.render(name.as_str(), context)?;
                let written = file.write(rendered.as_bytes())?;
                if written != rendered.len() {
                    return Err(anyhow!("written content does not match provided data size"));
                }
                Ok(())
            }
        }
    }
}

pub enum FileSystemCommand {
    Dir(String, Vec<FileSystemCommand>),
    File(String, FileContent<'static>),
    // Makefile(FileContent<'static>),
    // JS(String, FileContent<'static>),
    // CSS(String, FileContent<'static>),
    // Rust(String, FileContent<'static>),
}

impl Commander for FileSystemCommand {
    fn exec(&self, dest: path::PathBuf, value: &Value) -> FileResult<()> {
        match self {
            FileSystemCommand::Dir(dir, commands) => {
                let mut target_path = dest.clone();
                target_path.push(dir);

                let mut builder = fs::DirBuilder::new();
                builder.recursive(true).create(target_path.clone())?;

                for sub_command in commands {
                    sub_command.exec(target_path.clone(), value)?;
                }

                Ok(())
            }
            FileSystemCommand::File(file_name, content) => {
                let mut target_path = dest.clone();
                target_path.push(file_name);

                content.exec(target_path, Some(value))?;

                Ok(())
            }
        }
    }
}

pub trait Commander {
    fn exec(&self, dest: path::PathBuf, value: &Value) -> FileResult<()>;
}

pub struct Template {
    dest: path::PathBuf,
    commands: Vec<Box<dyn Commander>>,
}

impl Template {
    pub fn new<'a>(dest: &'a str) -> Self {
        Self {
            dest: path::PathBuf::from_str(dest).expect("created PathBuf from str"),
            commands: Vec::with_capacity(5),
        }
    }

    pub fn add(&mut self, command: Box<dyn Commander>) {
        self.commands.push(command);
    }

    pub fn run(&mut self, value: &Value) -> FileResult<()> {
        for command in self.commands.iter() {
            if let Err(err) = command.exec(self.dest.clone(), value) {
                return Err(err);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FileContent, FileResult, FileSystemCommand, Template};
    use serde_json::{json, Value};
    use std::{env, fs, io::Read, path};
    use tinytemplate::TinyTemplate;

    fn random_directory_name<'a>(prefix: &'a str) -> String {
        use rand::distributions::{Alphanumeric, DistString};

        return format!(
            "{}_{}",
            prefix,
            Alphanumeric.sample_string(&mut rand::thread_rng(), 16)
        );
    }

    fn clean_up_directory(target: path::PathBuf) {
        fs::remove_dir_all(target).expect("should have deleted directory");
    }

    fn create_template() -> FileResult<TinyTemplate<'static>> {
        let mut tt = TinyTemplate::new();

        tt.add_template("world", "{country} wonderworld!")?;
        tt.add_template("hello", "Welcome to hello {name}!")?;

        tt.add_template(
            "index",
            r#"{{ call hello with @root }} {{ call world with @root }}"#,
        )?;

        Ok(tt)
    }

    #[test]
    fn test_can_create_directory() {
        let tmp_dir = env::temp_dir();

        let mut target = tmp_dir.clone();
        target.push("temple");

        let mut tml = Template::new(&target.to_str().unwrap());
        tml.add(Box::new(FileSystemCommand::Dir(
            String::from("weeds"),
            vec![],
        )));

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
    fn test_can_create_directory_with_file() {
        let tmp_dir = env::temp_dir();

        let mut target = tmp_dir.clone();
        target.push(random_directory_name("temple"));

        let mut tml = Template::new(&target.to_str().unwrap());

        let templ = create_template().expect("created sample template");

        tml.add(Box::new(FileSystemCommand::Dir(
            String::from("weeds"),
            vec![FileSystemCommand::File(
                String::from("index.md"),
                FileContent::Template(String::from("index"), templ),
            )],
        )));

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
