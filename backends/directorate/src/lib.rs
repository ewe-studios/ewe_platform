// Provides wrappers for rust_embed asset managemer.

use std::marker::PhantomData;

pub type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub struct Directorate<T: rust_embed::RustEmbed> {
    pub _data: PhantomData<T>,
}

// -- constructor + default

impl<T: rust_embed::Embed + Default> Default for Directorate<T> {
    fn default() -> Self {
        Self {
            _data: PhantomData::default(),
        }
    }
}

// -- Rust Embed wrapper methods and constructor

pub trait PackageDirectorate {
    /// Returns the underlying content of a file.
    fn get_file(&self, target_file: &str) -> Option<rust_embed::EmbeddedFile>;

    /// as_vec returns all the files within the package directorate as a Vec<String>.
    fn as_vec(&self) -> Vec<String>;

    /// top_directories returns all top-level directories within package.
    fn top_directories(&self) -> Vec<String>;

    /// Returns all filenames in directorate.
    fn files(&self) -> rust_embed::Filenames;

    /// Returns all filenames for giving root directory.
    fn files_for(&self, directory: &str) -> Option<Vec<String>>;
}

impl<T: rust_embed::Embed + 'static> Into<Box<dyn PackageDirectorate>> for Directorate<T> {
    fn into(self) -> Box<dyn PackageDirectorate> {
        Box::new(self)
    }
}

impl<T: rust_embed::Embed> PackageDirectorate for Directorate<T> {
    fn get_file(&self, target_file: &str) -> Option<rust_embed::EmbeddedFile> {
        T::get(target_file)
    }

    fn files(&self) -> rust_embed::Filenames {
        T::iter()
    }

    fn top_directories(&self) -> Vec<String> {
        let mut dirs: Vec<String> = T::iter()
            .filter(|t| t.contains("/"))
            .map(|t| match t.split_once("/") {
                None => None,
                Some((directory, _)) => Some(String::from(directory)),
            })
            .filter(|t| t.is_some())
            .map(|t| t.unwrap())
            .collect();

        // sort and de-dup
        dirs.sort();
        dirs.dedup();

        dirs
    }

    fn as_vec(&self) -> Vec<String> {
        T::iter().map(|t| String::from(t)).collect()
    }

    fn files_for(&self, directory: &str) -> Option<Vec<String>> {
        let target_dir = if directory.ends_with("/") {
            directory
        } else {
            &format!("{}/", directory)
        };

        let files: Vec<String> = T::iter()
            .filter(|t| t.starts_with(target_dir))
            .map(|t| String::from(t))
            .collect();

        if files.is_empty() {
            return None;
        }

        Some(files)
    }
}

#[cfg(test)]
mod directorate_tests {

    use super::*;

    #[derive(rust_embed::Embed, Default)]
    #[folder = "test_directory/"]
    struct Directory;

    #[test]
    fn validate_can_create_directorate_generator_safely() {
        let generator = Directorate::<Directory>::default();
        assert!(matches!(generator.get_file("README.md"), Some(_)));
    }

    #[test]
    fn validate_can_read_top_directories() {
        let generator = Directorate::<Directory>::default();
        let directories: Vec<String> = generator.top_directories();
        assert_eq!(directories, vec! {"docs", "schema"});
    }

    #[test]
    fn validate_can_read_only_files_for_top_directory() {
        let generator = Directorate::<Directory>::default();
        let files: Option<Vec<String>> = generator.files_for("schema");
        assert_eq!(
            files.unwrap(),
            vec! {"schema/partials/partial_1.sql", "schema/schema.sql"}
        );
    }

    #[test]
    fn validate_can_read_all_directories() {
        let generator = Directorate::<Directory>::default();
        let files: Vec<String> = generator.files().map(|t| String::from(t)).collect();
        assert_eq!(
            files,
            vec! {"README.md", "docs/runner.sh", "elem.js", "schema/partials/partial_1.sql", "schema/schema.sql"}
        );
    }
}
