use foundation_macros::scaffold_impl;
use foundation_nativeapis::shared::vfs::{
    MemoryFs, OpenMode, VfsCapabilities, VfsFileSystem,
    VfsMetadata, VfsResult,
};

struct LoggingFs {
    inner: MemoryFs,
    label: String,
}

impl LoggingFs {
    fn new(label: &str) -> Self {
        Self {
            inner: MemoryFs::new(),
            label: label.to_string(),
        }
    }
}

#[scaffold_impl(via = "self.inner")]
impl VfsFileSystem for LoggingFs {
    type File = <MemoryFs as VfsFileSystem>::File;
    type SeekableFile = <MemoryFs as VfsFileSystem>::SeekableFile;
    type Directory = <MemoryFs as VfsFileSystem>::Directory;

    // Override create to add logging
    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        println!("[{}] create {:?} mode={:o}", self.label, path, mode);
        self.inner.create(path, mode)
    }

    // Override mkdir to add logging
    fn mkdir(&self, path: &str) -> VfsResult<()> {
        println!("[{}] mkdir {:?}", self.label, path);
        self.inner.mkdir(path)
    }

    // Everything else delegates to self.inner via scaffold!()
    fn capabilities(&self) -> VfsCapabilities { scaffold!() }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { scaffold!() }
    fn inode(&self, path: &str) -> VfsResult<u64> { scaffold!() }
    fn path_by_inode(&self, ino: u64) -> VfsResult<String> { scaffold!() }
    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> { scaffold!() }
    fn exists(&self, path: &str) -> VfsResult<bool> { scaffold!() }
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> { scaffold!() }
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> { scaffold!() }
    fn readlink(&self, path: &str) -> VfsResult<String> { scaffold!() }
    fn rename(&self, from: &str, to: &str) -> VfsResult<()> { scaffold!() }
    fn remove(&self, path: &str) -> VfsResult<()> { scaffold!() }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> { scaffold!() }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> { scaffold!() }
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> { scaffold!() }
}

fn main() {
    println!("=== scaffold!() Macro Example ===\n");
    println!("Demonstrates trait delegation via #[scaffold_impl] + scaffold!()");
    println!("Only create() and mkdir() are overridden; everything else delegates.\n");

    let fs = LoggingFs::new("audit");

    // These two call our overridden methods (with logging)
    fs.mkdir("/data").unwrap();
    fs.create("/data/report.csv", 0o644).unwrap();

    // These call the delegated scaffold!() methods (no logging, pass-through)
    fs.write_file("/data/report.csv", b"name,age\nalice,30\n").unwrap();
    let data = fs.read_file("/data/report.csv").unwrap();
    println!("\nRead back: {:?}", String::from_utf8_lossy(&data));

    let meta = fs.stat("/data/report.csv").unwrap();
    println!("stat: size={}, type={:?}", meta.size, meta.file_type);

    println!("\n=== Done ===");
}
