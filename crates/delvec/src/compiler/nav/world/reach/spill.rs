//! **The reach graph's scratch files**: where the structures the graph keeps
//! for the life of the process live when they are not being read — the node
//! bank's variants ([`super::bank`]), the per-chunk edge lists and the node
//! tables ([`super::chunks`]) and the snapshot's pages ([`super::pages`]).
//! Each record is appended once and read back by offset; what is in memory is
//! the offset.
//!
//! **Where**: the system's temporary directory (`std::env::temp_dir`, so
//! `TMPDIR` on Unix and `TMP` on Windows name it), never the output tree: the
//! output directory is the deliverable, hashed whole by the determinism gates,
//! and a `--perturb` build has none. A file is unlinked the moment it is open
//! (Unix) or marked to be deleted when it closes (Windows), so no run leaves
//! one behind.
//!
//! **Errors, one policy for every spill**: a file that cannot be opened is no
//! file, and the structure holds its records in memory instead (a slower run,
//! the same answers). A file that was opened and then refuses a write or a
//! read back is a failed build: the run stops where it stands, with a message
//! naming the file, the operation, the error and the bytes written so far
//! ([`Spill::fail`]). Nothing a build emits depends on a scratch file — what
//! is read back is what was written — so a half-read record is never
//! decoded into an answer.
//!
//! **Concurrency**: writes and reads are both positional (`pwrite`/`pread`,
//! `seek_write`/`seek_read`), so a reader cloned from a writer
//! ([`Spill::reader`]) can read while the writer appends, on Windows too,
//! where a duplicated handle shares the file pointer — neither side moves it
//! or trusts it. A writer still needs the caller's lock: two appends at once
//! would take the same offset.

use std::sync::Arc;

/// One scratch file, appended and read back by offset.
pub(crate) struct Spill {
    file: std::fs::File,
    len: u64,
    name: Arc<str>,
}

/// A reader of a [`Spill`]'s records, used without the writer's lock.
pub(crate) struct Reader {
    file: std::fs::File,
    name: Arc<str>,
}

impl Spill {
    /// A fresh scratch file in the system temporary directory, named for what
    /// it holds.
    pub(crate) fn open(what: &str) -> std::io::Result<Spill> {
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
        Ok(Spill {
            file,
            len: 0,
            name: Arc::from(path.to_string_lossy().as_ref()),
        })
    }

    /// Append `bytes`; where they lie. Positional: the file's cursor is
    /// neither moved nor read.
    pub(crate) fn append(&mut self, bytes: &[u8]) -> std::io::Result<(u64, u32)> {
        let at = self.len;
        let len = u32::try_from(bytes.len()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "one record under 4 GiB")
        })?;
        let mut done = 0;
        while done < bytes.len() {
            #[cfg(unix)]
            let n =
                std::os::unix::fs::FileExt::write_at(&self.file, &bytes[done..], at + done as u64)?;
            #[cfg(windows)]
            let n = std::os::windows::fs::FileExt::seek_write(
                &self.file,
                &bytes[done..],
                at + done as u64,
            )?;
            if n == 0 {
                return Err(std::io::ErrorKind::WriteZero.into());
            }
            done += n;
        }
        self.len += bytes.len() as u64;
        Ok((at, len))
    }

    /// Write `bytes` over a region written before, at `at`: the region's
    /// owner reuses it for a new record of the same array, so the file holds
    /// one region per array and not one per give-up. The region must lie
    /// within what was appended; positional, as [`Spill::append`] is.
    pub(crate) fn write_at(&mut self, at: u64, bytes: &[u8]) -> std::io::Result<()> {
        let end = at.saturating_add(bytes.len() as u64);
        if end > self.len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "a region rewritten lies within the file",
            ));
        }
        let mut done = 0;
        while done < bytes.len() {
            #[cfg(unix)]
            let n =
                std::os::unix::fs::FileExt::write_at(&self.file, &bytes[done..], at + done as u64)?;
            #[cfg(windows)]
            let n = std::os::windows::fs::FileExt::seek_write(
                &self.file,
                &bytes[done..],
                at + done as u64,
            )?;
            if n == 0 {
                return Err(std::io::ErrorKind::WriteZero.into());
            }
            done += n;
        }
        Ok(())
    }

    /// The `len` bytes at `at`, read at their offset without moving the
    /// file's cursor.
    pub(crate) fn read(&self, at: u64, len: u32) -> std::io::Result<Vec<u8>> {
        read_at(&self.file, at, len)
    }

    /// A reader of what is written, for use without the writer's lock.
    pub(crate) fn reader(&self) -> std::io::Result<Reader> {
        Ok(Reader {
            file: self.file.try_clone()?,
            name: Arc::clone(&self.name),
        })
    }

    /// The bytes written so far.
    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    /// **The one failure of an open scratch file**: stop the run, naming the
    /// file, what was being done to it, the error and the bytes written.
    pub(crate) fn fail(&self, op: &str, err: &std::io::Error) -> ! {
        fail(&self.name, op, Some(self.len), err)
    }

    /// `r`, or the run stopped by [`Spill::fail`].
    pub(crate) fn or_fail<T>(&self, op: &str, r: std::io::Result<T>) -> T {
        match r {
            Ok(v) => v,
            Err(e) => self.fail(op, &e),
        }
    }
}

impl Reader {
    /// The `len` bytes at `at`, or the run stopped by [`Spill::fail`]'s
    /// message.
    pub(crate) fn read(&self, at: u64, len: u32) -> Vec<u8> {
        match read_at(&self.file, at, len) {
            Ok(v) => v,
            Err(e) => fail(&self.name, &format!("read {len} byte(s) at {at}"), None, &e),
        }
    }
}

fn fail(name: &str, op: &str, written: Option<u64>, err: &std::io::Error) -> ! {
    let written = written.map_or(String::new(), |n| {
        format!("; {n} byte(s) had been written to it")
    });
    panic!(
        "the reach graph's scratch file {name} failed to {op} ({err}){written}. The build cannot \
         go on without the record: free the disk the system's temporary directory is on, or point \
         TMPDIR at another, and build again (ADR-0037)"
    )
}

/// The `len` bytes of `file` at `at`, read without moving its cursor.
fn read_at(file: &std::fs::File, at: u64, len: u32) -> std::io::Result<Vec<u8>> {
    let mut out = vec![0u8; len as usize];
    let mut done = 0;
    while done < out.len() {
        #[cfg(unix)]
        let n = std::os::unix::fs::FileExt::read_at(file, &mut out[done..], at + done as u64)?;
        #[cfg(windows)]
        let n = std::os::windows::fs::FileExt::seek_read(file, &mut out[done..], at + done as u64)?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        done += n;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::Spill;
    use std::sync::{Arc, Mutex};

    /// **Every record reads back as written while writers append and readers
    /// read at once** (ADR-0037 §2): four threads append records of varying
    /// length through one locked writer while four more read back, through
    /// an unlocked reader cloned from it, every slot published so far — on
    /// the shared handle whose cursor Windows moves on every read and would
    /// move on a cursor-relative write. Red on any record read back other
    /// than as written, and on a run whose readers never read a slot while a
    /// writer was still appending (which would bind nothing) — which the
    /// writers make structural: each stops halfway until the readers have
    /// taken a pass over the published slots, so an overlapping read happens
    /// however the threads are scheduled, under a loaded machine too.
    #[test]
    fn records_read_back_as_written_while_writers_and_readers_run_at_once() {
        let spill = Spill::open("test-spill").expect("a scratch file opens in the temp dir");
        let reader = spill.reader().expect("a reader");
        let writer = Arc::new(Mutex::new(spill));
        /// Offset, length, writer, record index: what each slot holds.
        type Slot = (u64, u32, u8, u32);
        let published: Arc<Mutex<Vec<Slot>>> = Default::default();
        let writing = Arc::new(std::sync::atomic::AtomicUsize::new(4));
        let overlapped = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let passes = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let record = |w: u8, i: u32| -> Vec<u8> {
            let n = 1 + ((i as usize * 7 + w as usize * 13) % 300);
            (0..n).map(|k| (k as u8) ^ w ^ (i as u8)).collect()
        };
        std::thread::scope(|s| {
            for w in 0..4u8 {
                let (writer, published, writing, passes) = (
                    Arc::clone(&writer),
                    Arc::clone(&published),
                    Arc::clone(&writing),
                    Arc::clone(&passes),
                );
                s.spawn(move || {
                    for i in 0..600u32 {
                        if i == 300 {
                            // Halfway: wait until the readers have read what
                            // is published while this writer still appends.
                            while passes.load(std::sync::atomic::Ordering::SeqCst) == 0 {
                                std::thread::yield_now();
                            }
                        }
                        let bytes = record(w, i);
                        let (at, len) = {
                            let mut g = writer.lock().expect("writer");
                            g.append(&bytes).expect("a write")
                        };
                        published.lock().expect("published").push((at, len, w, i));
                    }
                    writing.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                });
            }
            for _ in 0..4 {
                let (published, writing, overlapped, passes, reader) = (
                    Arc::clone(&published),
                    Arc::clone(&writing),
                    Arc::clone(&overlapped),
                    Arc::clone(&passes),
                    &reader,
                );
                s.spawn(move || {
                    loop {
                        let still = writing.load(std::sync::atomic::Ordering::SeqCst) > 0;
                        let slots = published.lock().expect("published").clone();
                        for (at, len, w, i) in &slots {
                            assert_eq!(
                                reader.read(*at, *len),
                                record(*w, *i),
                                "slot at {at} ({len} byte(s)) of writer {w}, record {i}"
                            );
                        }
                        if still {
                            overlapped.fetch_add(
                                slots.len() as u64,
                                std::sync::atomic::Ordering::Relaxed,
                            );
                            if !slots.is_empty() {
                                passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            }
                        } else {
                            break;
                        }
                    }
                });
            }
        });
        let slots = published.lock().expect("published");
        let g = writer.lock().expect("writer");
        for (at, len, w, i) in slots.iter() {
            assert_eq!(g.read(*at, *len).expect("a read"), record(*w, *i));
        }
        let overlapped = overlapped.load(std::sync::atomic::Ordering::Relaxed);
        eprintln!(
            "spill: {} record(s) over {} byte(s); {overlapped} read(s) back while a writer was still \
             appending",
            slots.len(),
            g.len()
        );
        assert_eq!(slots.len(), 2400);
        assert!(
            overlapped > 0,
            "no read overlapped a write: the case binds nothing"
        );
    }
}
