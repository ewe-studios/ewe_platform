use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// A helper function for recursively copying a directory.
fn copy_dir<P, Q>(from: P, to: Q)
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let to = to.as_ref().to_path_buf();

    for path in fs::read_dir(from).unwrap() {
        let path = path.unwrap().path();
        let to = to.clone().join(path.file_name().unwrap());

        if path.is_file() {
            fs::copy(&path, to).unwrap();
            continue;
        }

        if path.is_dir() {
            if !to.exists() {
                fs::create_dir(&to).unwrap();
            }

            copy_dir(&path, to);
        }
    }
}

fn main() {
    // Request the output directory

    let template_dir = env::var("TEMPLATES_DIR").unwrap();
    println!("TEMPLATE_DIR: {template_dir:?}");

    let out_directory = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("OUT_DIR: {out_directory:?}");

    let output_directory = out_directory.join("templates");

    let package_directory = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    println!("PACKAGE_DIRECTORY: {package_directory:?}");

    let source_directory = package_directory.join(template_dir);
    println!("SOURCE_DIRECTORY: {source_directory:?}");

    // If it is already in the output directory, delete it and start over
    if output_directory.exists() {
        fs::remove_dir_all(&output_directory).unwrap();
    }

    // Create the out directory
    fs::create_dir(&output_directory).unwrap();

    // Copy the directory
    copy_dir(source_directory, &output_directory);
}
