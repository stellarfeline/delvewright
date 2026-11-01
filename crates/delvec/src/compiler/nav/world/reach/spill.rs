//! **The reach graph's scratch file**: where the structures the graph keeps for
//! the life of the process live when they are not being read — the node bank's
//! variants ([`super::bank`]) and the per-chunk edge lists of the node tables
//! and the closures ([`super::chunks`]). Each is appended once and read back by
//! offset; what is in memory is the offset.
//!
//! A file is unlinked the moment it is open (Unix) or marked to be deleted when
//! it closes (Windows), so no run leaves one behind. Nothing a build emits
//! depends on it: what is read back is what was written, and a structure whose
//! file cannot be opened is held in memory instead.

/// One scratch file, appended and read back by offset.
pub(super) struct Spill {
    file: std::fs::File,
    len: u64,
}

impl Spill {
    /// A fresh scratch file in the system temporary directory, named for what
    /// it holds.
    pub(super) fn open(what: &str) -> std::io::Result<Spill> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "delvec-reach-{what}-{}-{n}.spill",
            std::process::id()
        ));
        let mut o = std::fs::OpenOptions::new();
        o.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_FLAG_DELETE_ON_CLOSE
            o.custom_flags(0x0400_0000);
        }
        let file = o.open(&path)?;
        #[cfg(unix)]
        std::fs::remove_file(&path)?;
        Ok(Spill { file, len: 0 })
    }

    /// Append `bytes`; where they lie.
    pub(super) fn append(&mut self, bytes: &[u8]) -> std::io::Result<(u64, u32)> {
        use std::io::{Seek, Write};
        let at = self.len;
        let len = u32::try_from(bytes.len()).expect("one record under 4 GiB");
        self.file.seek(std::io::SeekFrom::Start(at))?;
        self.file.write_all(bytes)?;
        self.len += bytes.len() as u64;
        Ok((at, len))
    }

    /// The `len` bytes at `at`.
    pub(super) fn read(&mut self, at: u64, len: u32) -> std::io::Result<Vec<u8>> {
        use std::io::{Read, Seek};
        let mut out = vec![0u8; len as usize];
        self.file.seek(std::io::SeekFrom::Start(at))?;
        self.file.read_exact(&mut out)?;
        Ok(out)
    }

    /// The bytes written so far.
    pub(super) fn len(&self) -> u64 {
        self.len
    }
}
