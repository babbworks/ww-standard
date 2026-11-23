//! Where a log's bytes live.
//!
//! The log's rules never touch storage directly. They read and append
//! segments, and read and write content stored by hash, through [`Store`].
//! [`DirStore`] keeps a log in a directory (`seg-000001.log`, …, and
//! `blobs/<sha256>`); [`MemStore`] keeps it in memory, for the browser and
//! for tests. Both hold the same bytes, so a log can move between them.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub trait Store {
    /// The bytes of segment `n` (from 1), or `None` if it doesn't exist.
    fn segment(&self, n: u32) -> io::Result<Option<Vec<u8>>>;
    /// Append bytes to segment `n`, creating it if needed.
    fn append(&mut self, n: u32, bytes: &[u8]) -> io::Result<()>;
    /// Content stored by hash, or `None`.
    fn blob(&self, hash: &str) -> io::Result<Option<Vec<u8>>>;
    /// Store content under its hash. Storing the same content twice is a no-op.
    fn put_blob(&mut self, hash: &str, bytes: &[u8]) -> io::Result<()>;
    /// Where the log is, for messages.
    fn location(&self) -> String;
}

/// A log in a directory. One writer at a time.
pub struct DirStore {
    dir: PathBuf,
}

impl DirStore {
    pub fn new(dir: &Path) -> DirStore {
        DirStore { dir: dir.to_path_buf() }
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    fn seg_path(&self, n: u32) -> PathBuf {
        self.dir.join(format!("seg-{n:06}.log"))
    }
}

fn read_opt(p: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(p) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

impl Store for DirStore {
    fn segment(&self, n: u32) -> io::Result<Option<Vec<u8>>> {
        read_opt(&self.seg_path(n))
    }
    fn append(&mut self, n: u32, bytes: &[u8]) -> io::Result<()> {
        let mut f = OpenOptions::new().create(true).append(true).open(self.seg_path(n))?;
        f.write_all(bytes)?;
        f.sync_data()
    }
    fn blob(&self, hash: &str) -> io::Result<Option<Vec<u8>>> {
        read_opt(&self.dir.join("blobs").join(hash))
    }
    fn put_blob(&mut self, hash: &str, bytes: &[u8]) -> io::Result<()> {
        let dir = self.dir.join("blobs");
        fs::create_dir_all(&dir)?;
        let p = dir.join(hash);
        if !p.exists() {
            fs::write(p, bytes)?;
        }
        Ok(())
    }
    fn location(&self) -> String {
        self.dir.display().to_string()
    }
}

/// A log held in memory.
#[derive(Default, Clone)]
pub struct MemStore {
    pub segments: Vec<Vec<u8>>,
    pub blobs: BTreeMap<String, Vec<u8>>,
}

impl Store for MemStore {
    fn segment(&self, n: u32) -> io::Result<Option<Vec<u8>>> {
        Ok(n.checked_sub(1).and_then(|i| self.segments.get(i as usize)).cloned())
    }
    fn append(&mut self, n: u32, bytes: &[u8]) -> io::Result<()> {
        let i = n as usize - 1;
        if i > self.segments.len() {
            return Err(io::Error::other(format!("segment {n} would leave a hole")));
        }
        if i == self.segments.len() {
            self.segments.push(Vec::new());
        }
        self.segments[i].extend_from_slice(bytes);
        Ok(())
    }
    fn blob(&self, hash: &str) -> io::Result<Option<Vec<u8>>> {
        Ok(self.blobs.get(hash).cloned())
    }
    fn put_blob(&mut self, hash: &str, bytes: &[u8]) -> io::Result<()> {
        self.blobs.entry(hash.to_string()).or_insert_with(|| bytes.to_vec());
        Ok(())
    }
    fn location(&self) -> String {
        "memory".into()
    }
}
