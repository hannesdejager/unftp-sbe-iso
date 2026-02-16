//! A [libunftp](https://docs.rs/libunftp/latest/libunftp/)
//! storage back-end that allows FTP access to ".iso" (ISO 9660) files.
//!
//! ## Usage
//!
//! Add the `libunftp` and `tokio` crates to your project's dependencies in Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! libunftp = "0.20.3"
//! unftp-sbe-iso = "0.1"
//! tokio = { version = "1", features = ["full"] }
//! ```
//!
//! Add the following to src/main.rs:
//!
//! ```no_run
//! use libunftp::ServerBuilder;
//! use unftp_sbe_iso::Storage;
//
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() {
//!     let addr = "127.0.0.1:2121";
//!
//!     let server = ServerBuilder::new(Box::new(move || Storage::new("/path/to/your/image.iso")))
//!         .greeting("Welcome to my ISO over FTP")
//!         .passive_ports(50000..=65535)
//!         .build()
//!         .unwrap();
//!
//!     println!("Starting FTP server on {}", addr);
//!     server.listen(addr).await.unwrap();
//! }
//! ```
//!
//! You can now run your server with cargo run and connect to localhost:2121 with your favourite FTP client e.g.:
//!
//! ```sh
//! lftp localhost -p 2121
//! ```

use async_trait::async_trait;
use cdfs::{DirectoryEntry, ExtraAttributes, ISO9660, ISODirectory, ISOFileReader};
use std::{
    fmt::Debug,
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};
use tokio::io::AsyncRead;
use unftp_core::{
    auth::UserDetail,
    storage::{Error, ErrorKind, Fileinfo, Metadata, Result, StorageBackend},
};

/// A virtual file system that tells the unftp server how to access ".iso" (ISO 9660) files.
#[derive(Debug, Clone)]
pub struct Storage {
    iso_path: PathBuf,
}

impl Storage {
    /// Creates the storage back-end, pointing it to the ".iso" file
    /// given in the `iso_path` parameter.
    pub fn new<P: AsRef<Path>>(iso_path: P) -> Self {
        Self {
            iso_path: iso_path.as_ref().to_path_buf(),
        }
    }

    fn open_iso(&self) -> std::io::Result<ISO9660<std::fs::File>> {
        let file = std::fs::File::open(&self.iso_path)?;
        Ok(ISO9660::new(file).unwrap())
    }

    fn find<P: AsRef<Path> + Send + Debug>(&self, path: P) -> Result<DirectoryEntry<File>> {
        let iso: ISO9660<File> = self.open_iso()?;
        let mut current_dir: ISODirectory<File> = iso.root().clone();

        let mut components = path.as_ref().components().peekable();

        while let Some(comp) = components.next() {
            use std::path::Component;

            let name = match comp {
                Component::RootDir => continue,
                Component::Normal(name) => name.to_str().unwrap().to_uppercase(),
                _ => {
                    return Err(Error::new(
                        ErrorKind::PermanentFileNotAvailable,
                        "Unsupported path component",
                    ));
                }
            };

            // Find the next entry in the current directory
            let next_entry: DirectoryEntry<File> = current_dir
                .contents()
                .filter_map(|e| e.ok())
                .find(|e| e.identifier().eq_ignore_ascii_case(&name))
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::TransientFileNotAvailable,
                        format!("Path component '{}' not found", name),
                    )
                })?;

            if components.peek().is_none() {
                // This is the last component — return the entry
                return Ok(next_entry);
            }

            // Not the last component — must be a directory
            match next_entry {
                DirectoryEntry::Directory(dir) => {
                    current_dir = dir; // move the directory, no borrow
                }
                _ => {
                    return Err(Error::new(
                        ErrorKind::PermanentFileNotAvailable,
                        "Intermediate path component is not a directory",
                    ));
                }
            }
        }

        // If we get here, it means the path was `/` or empty — return root dir entry
        Ok(DirectoryEntry::Directory(current_dir))
    }
}

#[async_trait]
impl<User: UserDetail> StorageBackend<User> for Storage {
    type Metadata = IsoMeta;

    async fn metadata<P: AsRef<Path> + Send + Debug>(
        &self,
        _user: &User,
        path: P,
    ) -> Result<Self::Metadata> {
        let entry = self.find(path)?;
        let size = match &entry {
            DirectoryEntry::Directory(d) => d.header().length as u64,
            DirectoryEntry::File(f) => f.size() as u64,
            DirectoryEntry::Symlink(l) => l.header().length as u64,
        };
        Ok(IsoMeta {
            len: size,
            dir: matches!(entry, DirectoryEntry::Directory(_)),
            sym: matches!(entry, DirectoryEntry::Symlink(_)),
            group: entry.group().unwrap_or(0),
            owner: entry.owner().unwrap_or(0),
            modified: entry.modify_time().into(),
        })
    }

    async fn list<P: AsRef<Path> + Send + Debug>(
        &self,
        _user: &User,
        path: P,
    ) -> Result<Vec<Fileinfo<PathBuf, Self::Metadata>>>
    where
        <Self as StorageBackend<User>>::Metadata: Metadata,
    {
        let mut entries = Vec::new();
        let e = self.find(path)?;
        let d = match e {
            DirectoryEntry::Directory(d) => d,
            DirectoryEntry::File(_) => return Err(Error::from(ErrorKind::FileNameNotAllowedError)),
            DirectoryEntry::Symlink(_) => {
                return Err(Error::from(ErrorKind::FileNameNotAllowedError));
            }
        };
        for entry in d.contents() {
            let e = entry.unwrap();
            let size = match &e {
                DirectoryEntry::Directory(d) => d.header().length as u64,
                DirectoryEntry::File(f) => f.size() as u64,
                DirectoryEntry::Symlink(l) => l.header().length as u64,
            };
            entries.push(Fileinfo {
                path: e.identifier().into(),
                metadata: IsoMeta {
                    len: size,
                    dir: matches!(e, DirectoryEntry::Directory(_)),
                    sym: matches!(e, DirectoryEntry::Symlink(_)),
                    group: e.group().unwrap_or(0),
                    owner: e.owner().unwrap_or(0),
                    modified: e.modify_time().into(),
                },
            });
        }
        Ok(entries)
    }

    async fn get<P: AsRef<Path> + Send + Debug>(
        &self,
        _user: &User,
        path: P,
        start_pos: u64,
    ) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>> {
        let entry: DirectoryEntry<File> = self.find(path)?;
        match entry {
            DirectoryEntry::File(file_entry) => {
                let mut reader: ISOFileReader<File> = file_entry.read();
                // Seek to the requested start position
                if start_pos > 0 {
                    reader.seek(SeekFrom::Start(start_pos)).map_err(|e| {
                        Error::new(
                            ErrorKind::PermanentFileNotAvailable,
                            format!("seek error: {e}"),
                        )
                    })?;
                }

                // Read entire contents into a Vec<u8>
                let mut buf = Vec::new();
                reader.read_to_end(&mut buf).map_err(|e| {
                    Error::new(
                        ErrorKind::PermanentFileNotAvailable,
                        format!("read error: {e}"),
                    )
                })?;

                // Return a cursor over the buffer to provide async access
                let cursor = Cursor::new(buf);
                Ok(Box::new(cursor))
            }

            DirectoryEntry::Directory(_) => Err(ErrorKind::PermanentFileNotAvailable.into()),
            DirectoryEntry::Symlink(_) => Err(ErrorKind::PermanentFileNotAvailable.into()),
        }
    }

    async fn put<P: AsRef<Path> + Send + Debug, R: AsyncRead + Send + Sync + Unpin + 'static>(
        &self,
        _user: &User,
        _input: R,
        _path: P,
        _start_pos: u64,
    ) -> Result<u64> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn del<P: AsRef<Path> + Send + Debug>(&self, _user: &User, _path: P) -> Result<()> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn mkd<P: AsRef<Path> + Send + Debug>(&self, _user: &User, _path: P) -> Result<()> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn rename<P: AsRef<Path> + Send + Debug>(
        &self,
        _user: &User,
        _from: P,
        _to: P,
    ) -> Result<()> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn rmd<P: AsRef<Path> + Send + Debug>(&self, _user: &User, _path: P) -> Result<()> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn cwd<P: AsRef<Path> + Send + Debug>(&self, _user: &User, path: P) -> Result<()> {
        self.find(path).map(|_d| ())
    }
}

/// Implements unftp-core's Metadata trait
#[derive(Debug)]
pub struct IsoMeta {
    /// The file size in bytes
    pub len: u64,
    /// Is it a directory?
    pub dir: bool,
    /// Is it a symbolic link?
    pub sym: bool,
    /// The Unix group ID if available, otherwise 0
    pub group: u32,
    /// The Unix UID if available, otherwise 0
    pub owner: u32,
    /// The last modified time of the file
    pub modified: SystemTime,
}

impl Metadata for IsoMeta {
    fn len(&self) -> u64 {
        self.len
    }

    fn is_dir(&self) -> bool {
        self.dir
    }

    fn is_file(&self) -> bool {
        !self.dir
    }

    fn is_symlink(&self) -> bool {
        false
    }

    fn modified(&self) -> Result<SystemTime> {
        Ok(self.modified)
    }

    fn gid(&self) -> u32 {
        self.group
    }

    fn uid(&self) -> u32 {
        self.owner
    }
}
