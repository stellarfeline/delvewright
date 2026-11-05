//! **The reach graph's scratch files**: where the structures the graph keeps
//! for the life of the process live when they are not being read — the node
//! bank's variants ([`super::bank`]), the per-chunk edge lists and the node
//! tables ([`super::chunks`]) and the snapshot's pages ([`super::pages`]).
//! Each record is written once and read back by offset; what is in memory is
//! the offset.
//!
//! **Room**: a record the structure no longer holds ([`Extent`]'s drop — a
//! list's chunk rewritten whole, a list or a table dropped, a variant the bank
//! lets go) frees its bytes, the next record that fits is written into the
//! smallest free range, and a free range that reaches the file's end shortens
//! the file. A file is as long as what its structure holds, not the sum of
//! what it wrote — the lists rewrite a chunk whole each time it passes four
//! pieces, so the sum is many times what they hold. A record kept for the
//! file's life ([`Spill::append`]) never frees.
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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

/// One scratch file, appended and read back by offset.
pub(crate) struct Spill {
    file: std::fs::File,
    space: Arc<Mutex<Space>>,
    name: Arc<str>,
}

/// A reader of a [`Spill`]'s records, used without the writer's lock.
pub(crate) struct Reader {
    file: std::fs::File,
    space: Arc<Mutex<Space>>,
    name: Arc<str>,
}

/// **Where a file's records lie, and the room between them**: the file's end,
/// the free ranges below it, and what the file has held. A record given up
/// ([`Extent`]'s drop, [`Reader::give_back`]) leaves a free range; the next
/// record that fits takes the smallest that does, at its start; a free range
/// that reaches the end shortens the file to the last record still kept. So
/// the file is as long as what is kept, plus the free ranges no record has
/// fitted since — never the sum of everything ever written.
struct Space {
    /// A handle the file is shortened through.
    file: Option<std::fs::File>,
    /// One past the last byte any record kept holds: the file's length.
    end: u64,
    /// Free ranges below `end`, by offset: offset → length. Never two
    /// adjacent.
    by_at: BTreeMap<u64, u64>,
    /// The same ranges by (length, offset): the smallest that fits, first.
    by_len: BTreeSet<(u64, u64)>,
    /// Bytes held by records kept now.
    kept: u64,
    /// Bytes written over the file's life.
    written: u64,
    /// The longest the file has been.
    longest: u64,
}

impl Space {
    fn new(file: Option<std::fs::File>) -> Space {
        Space {
            file,
            end: 0,
            by_at: BTreeMap::new(),
            by_len: BTreeSet::new(),
            kept: 0,
            written: 0,
            longest: 0,
        }
    }

    /// Room for `len` bytes: the smallest free range that holds them, at its
    /// start, or the file's end.
    fn take(&mut self, len: u64) -> u64 {
        self.kept += len;
        self.written += len;
        if len > 0
            && let Some(&(flen, fat)) = self.by_len.range((len, 0)..).next()
        {
            self.by_len.remove(&(flen, fat));
            self.by_at.remove(&fat);
            if flen > len {
                self.insert(fat + len, flen - len);
            }
            return fat;
        }
        let at = self.end;
        self.end += len;
        self.longest = self.longest.max(self.end);
        at
    }

    fn insert(&mut self, at: u64, len: u64) {
        self.by_at.insert(at, len);
        self.by_len.insert((len, at));
    }

    fn remove(&mut self, at: u64) -> u64 {
        let len = self.by_at.remove(&at).expect("a free range");
        self.by_len.remove(&(len, at));
        len
    }

    /// Give `len` bytes at `at` back: joined to the free ranges either side,
    /// and the file shortened when the range reaches its end.
    fn give(&mut self, mut at: u64, mut len: u64) {
        if len == 0 {
            return;
        }
        self.kept -= len;
        if let Some((&pat, &plen)) = self.by_at.range(..at).next_back()
            && pat + plen == at
        {
            self.remove(pat);
            at = pat;
            len += plen;
        }
        if self.by_at.contains_key(&(at + len)) {
            len += self.remove(at + len);
        }
        if at + len == self.end {
            self.end = at;
            // A file that will not shorten is only longer than it need be:
            // what it holds is still read back by offset.
            if let Some(f) = &self.file {
                let _ = f.set_len(at);
            }
        } else {
            self.insert(at, len);
        }
    }
}

/// **A record kept in a [`Spill`]**: where it lies. Its bytes are the file's
/// until it is dropped, and then free for the next record (`Space`).
pub(crate) struct Extent {
    at: u64,
    len: u32,
    space: Arc<Mutex<Space>>,
}

impl Extent {
    /// Where the record lies: its offset and length.
    pub(crate) fn at(&self) -> (u64, u32) {
        (self.at, self.len)
    }
}

impl Drop for Extent {
    fn drop(&mut self) {
        // A poisoned lock means a run already stopping: nothing to give back.
        if let Ok(mut s) = self.space.lock() {
            s.give(self.at, u64::from(self.len));
        }
    }
}

impl std::fmt::Debug for Extent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Extent {{ at: {}, len: {} }}", self.at, self.len)
    }
}

/// What a scratch file has held: the bytes its kept records hold now, the
/// bytes written over its life, its length now and the longest it has been.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Usage {
    pub kept: u64,
    pub written: u64,
    pub len: u64,
    pub longest: u64,
}

impl std::ops::Add for Usage {
    type Output = Usage;
    fn add(self, o: Usage) -> Usage {
        Usage {
            kept: self.kept + o.kept,
            written: self.written + o.written,
            len: self.len + o.len,
            longest: self.longest + o.longest,
        }
    }
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
        let shorten = file.try_clone().ok();
        Ok(Spill {
            file,
            space: Arc::new(Mutex::new(Space::new(shorten))),
            name: Arc::from(path.to_string_lossy().as_ref()),
        })
    }

    /// Write `bytes` where there is room ([`Space`]); where they lie. The
    /// record is the file's while the extent lives, and its room is free once
    /// it is dropped. Positional: the file's cursor is neither moved nor read.
    pub(crate) fn put(&mut self, bytes: &[u8]) -> std::io::Result<Extent> {
        let len = u32::try_from(bytes.len()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "one record under 4 GiB")
        })?;
        let at = self.space.lock().expect("spill space").take(u64::from(len));
        // Held from here, so a write that fails gives its room back.
        let e = Extent {
            at,
            len,
            space: Arc::clone(&self.space),
        };
        write_at(&self.file, bytes, at)?;
        Ok(e)
    }

    /// Append `bytes`, kept for the file's life; where they lie.
    pub(crate) fn append(&mut self, bytes: &[u8]) -> std::io::Result<(u64, u32)> {
        let e = self.put(bytes)?;
        let at = e.at();
        std::mem::forget(e);
        Ok(at)
    }

    /// Write `bytes` over a region [`Spill::append`] kept, at `at`: the
    /// region's owner reuses it for a new record of the same array, so the
    /// file holds one region per array and not one per give-up. The region
    /// must lie within the file's length; a record kept for the file's life
    /// is never given back, so no free range and no shortening reaches it.
    /// Positional, as the append is.
    pub(crate) fn write_at(&mut self, at: u64, bytes: &[u8]) -> std::io::Result<()> {
        let end = at.saturating_add(bytes.len() as u64);
        if end > self.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "a region rewritten lies within the file",
            ));
        }
        write_at(&self.file, bytes, at)
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
            space: Arc::clone(&self.space),
            name: Arc::clone(&self.name),
        })
    }

    /// The file's length: one past the last byte a record kept holds.
    pub(crate) fn len(&self) -> u64 {
        self.space.lock().expect("spill space").end
    }

    /// What the file has held ([`Usage`]).
    pub(crate) fn usage(&self) -> Usage {
        let s = self.space.lock().expect("spill space");
        Usage {
            kept: s.kept,
            written: s.written,
            len: s.end,
            longest: s.longest,
        }
    }

    /// **The one failure of an open scratch file**: stop the run, naming the
    /// file, what was being done to it, the error and the file's length.
    pub(crate) fn fail(&self, op: &str, err: &std::io::Error) -> ! {
        let u = self.space.lock().map(|s| (s.end, s.written)).ok();
        fail(&self.name, op, u, err)
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

    /// Give back the room of a record [`Spill::append`] kept: the caller's
    /// promise that nothing reads it again.
    pub(crate) fn give_back(&self, at: u64, len: u32) {
        if let Ok(mut s) = self.space.lock() {
            s.give(at, u64::from(len));
        }
    }
}

fn fail(name: &str, op: &str, file: Option<(u64, u64)>, err: &std::io::Error) -> ! {
    let file = file.map_or(String::new(), |(len, written)| {
        format!("; the file stood at {len} byte(s), {written} written to it over the run")
    });
    panic!(
        "the reach graph's scratch file {name} failed to {op} ({err}){file}. The build cannot \
         go on without the record: free the disk the system's temporary directory is on, or point \
         TMPDIR at another, and build again (ADR-0037)"
    )
}

/// Write `bytes` at `at`, without moving the file's cursor.
fn write_at(file: &std::fs::File, bytes: &[u8], at: u64) -> std::io::Result<()> {
    let mut done = 0;
    while done < bytes.len() {
        #[cfg(unix)]
        let n = std::os::unix::fs::FileExt::write_at(file, &bytes[done..], at + done as u64)?;
        #[cfg(windows)]
        let n = std::os::windows::fs::FileExt::seek_write(file, &bytes[done..], at + done as u64)?;
        if n == 0 {
            return Err(std::io::ErrorKind::WriteZero.into());
        }
        done += n;
    }
    Ok(())
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

    /// **A record given up is room for the next, and the file ends at the
    /// last record kept** (ADR-0037 §2): three records written, the middle
    /// one dropped and a shorter one written into its room; the last dropped,
    /// and the file shortens on disk to the end of what is still kept; every
    /// record dropped, and the file is empty. Red on a dropped record whose
    /// room is never taken again, or a file that keeps its length.
    #[test]
    fn a_record_given_up_is_room_for_the_next_and_the_file_ends_at_the_last_kept() {
        let mut s = Spill::open("test-room").expect("a scratch file opens in the temp dir");
        let on_disk = |s: &Spill| s.file.metadata().expect("metadata").len();
        let a = s.put(&[1; 100]).expect("a write");
        let b = s.put(&[2; 200]).expect("a write");
        let c = s.put(&[3; 300]).expect("a write");
        assert_eq!((a.at(), b.at(), c.at()), ((0, 100), (100, 200), (300, 300)));
        drop(b);
        let d = s.put(&[4; 150]).expect("a write");
        assert_eq!(
            d.at(),
            (100, 150),
            "the dropped record's room is taken first"
        );
        assert_eq!(s.read(0, 100).expect("a read"), vec![1; 100]);
        assert_eq!(s.read(100, 150).expect("a read"), vec![4; 150]);
        assert_eq!(on_disk(&s), 600);
        drop(c);
        assert_eq!(s.len(), 250, "the file ends at the last record kept");
        assert_eq!(on_disk(&s), 250, "the file is shortened on disk");
        drop(a);
        drop(d);
        assert_eq!((s.len(), on_disk(&s)), (0, 0));
        let u = s.usage();
        assert_eq!((u.kept, u.written, u.longest), (0, 750, 600));
    }

    /// **A file written and given up endlessly stays as long as what is kept
    /// at once** (ADR-0037 §2): 20,000 records of 1 to 4,096 bytes, at most
    /// 64 kept at a time, each dropped in a generated order. The file is never
    /// longer than four times the most bytes kept at once; it would grow with
    /// every record written if dropped records left no room. Red on that
    /// growth.
    #[test]
    fn a_file_written_and_given_up_endlessly_stays_as_long_as_what_is_kept() {
        let mut s = Spill::open("test-churn").expect("a scratch file opens in the temp dir");
        let mut seed = 0x243f_6a88_85a3_08d3u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let mut kept: Vec<(super::Extent, u8)> = Vec::new();
        let mut most = 0;
        for i in 0..20_000u32 {
            if kept.len() == 64 || (!kept.is_empty() && next(3) == 0) {
                let (e, b) = kept.swap_remove(next(kept.len() as u64) as usize);
                let (at, len) = e.at();
                assert_eq!(s.read(at, len).expect("a read"), vec![b; len as usize]);
            }
            let b = i as u8;
            let len = 1 + next(4096) as usize;
            kept.push((s.put(&vec![b; len]).expect("a write"), b));
            // Counted by the test, never by the file's own accounting.
            most = most.max(kept.iter().map(|(e, _)| u64::from(e.at().1)).sum::<u64>());
        }
        let u = s.usage();
        eprintln!(
            "churn: {} byte(s) written, at most {most} kept at once, the file at most {} long",
            u.written, u.longest
        );
        assert!(
            u.longest <= 4 * most,
            "the file grew to {} byte(s) over at most {most} kept at once",
            u.longest
        );
        drop(kept);
        assert_eq!(s.len(), 0);
    }

    /// **A record kept for the file's life is never reached by the room
    /// given back** (ADR-0037 §1, §3; the section stores' regions): regions
    /// appended and rewritten in place ([`Spill::append`], [`Spill::write_at`])
    /// interleaved with records put and dropped ([`Spill::put`]) — the room
    /// the dropped records free is taken by later records and the file
    /// shortens behind the last record kept, and no free range, no later
    /// record and no shortening touches a region: every region reads back as
    /// it was last rewritten, the file never ends under a region, and a
    /// rewrite past the file's end is refused. Red on a region overwritten,
    /// a file shortened under one, or a rewrite past the end accepted.
    #[test]
    fn a_region_kept_for_the_files_life_is_never_reached_by_the_room_given_back() {
        let mut s = Spill::open("test-regions").expect("a scratch file opens in the temp dir");
        let on_disk = |s: &Spill| s.file.metadata().expect("metadata").len();
        const REGION: usize = 256;
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        // (offset, the byte the region was last rewritten with)
        let mut regions: Vec<(u64, u8)> = Vec::new();
        let mut dropped: Vec<super::Extent> = Vec::new();
        let (mut rewrites, mut given_back, mut reused) = (0u32, 0u32, 0u32);
        for i in 0..4_000u32 {
            match next(4) {
                0 => {
                    let b = i as u8;
                    let (at, _) = s.append(&[b; REGION]).expect("an append");
                    regions.push((at, b));
                }
                1 if !regions.is_empty() => {
                    let k = next(regions.len() as u64) as usize;
                    let b = i as u8;
                    s.write_at(regions[k].0, &[b; REGION]).expect("a rewrite");
                    regions[k].1 = b;
                    rewrites += 1;
                }
                2 => {
                    let len = 1 + next(3 * REGION as u64) as usize;
                    let e = s.put(&vec![i as u8; len]).expect("a put");
                    // A record written into room given back lies under the end.
                    if e.at().0 + u64::from(e.at().1) < s.len() {
                        reused += 1;
                    }
                    dropped.push(e);
                }
                _ if !dropped.is_empty() => {
                    drop(dropped.swap_remove(next(dropped.len() as u64) as usize));
                    given_back += 1;
                }
                _ => {}
            }
            let end = s.len();
            for (at, _) in &regions {
                assert!(
                    at + REGION as u64 <= end,
                    "step {i}: the file ends at {end}, under the region at {at}"
                );
            }
        }
        assert!(
            regions.len() > 100 && rewrites > 100 && given_back > 100 && reused > 100,
            "the case binds nothing: {} region(s), {rewrites} rewrite(s), {given_back} given \
             back, {reused} reused",
            regions.len()
        );
        for (at, b) in &regions {
            assert_eq!(
                s.read(*at, REGION as u32).expect("a read"),
                vec![*b; REGION],
                "the region at {at} reads back other than as last rewritten"
            );
        }
        drop(dropped);
        let last = regions
            .iter()
            .map(|(at, _)| at + REGION as u64)
            .max()
            .unwrap_or(0);
        assert_eq!(
            (s.len(), on_disk(&s)),
            (last, last),
            "with every put record dropped, the file ends at the last region kept"
        );
        assert!(
            s.write_at(last - 1, &[0; 2]).is_err(),
            "a rewrite past the end is refused"
        );
        assert_eq!(
            s.read(last - 1, 1).expect("a read"),
            vec![
                regions
                    .iter()
                    .find(|(at, _)| at + REGION as u64 == last)
                    .expect("the last region")
                    .1
            ]
        );
        eprintln!(
            "regions: {} kept, {rewrites} rewrite(s), {given_back} record(s) given back, {reused} \
             written into room given back, the file at most {} long",
            regions.len(),
            s.usage().longest
        );
    }
}
