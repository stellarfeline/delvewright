//! **The node bank across runs**: where `DELVEC_REACH_CACHE` names a
//! directory, every bank ([`super::bank`]) is read from it when the run first
//! asks for it and written back to it when the run ends, so a later run over
//! a world of the same window takes the nodes this one flew.
//!
//! A bank holds each node it flew with every cell its expansion read and what
//! each held ([`super::bank`]); a later graph takes a node only where its world
//! holds the same at every one of them. That rule does not care which run
//! wrote the node, so carrying the bank across runs changes how long a build
//! takes and never what it emits (ADR-0006). What the rule does not cover is
//! the engine itself: a node flown by other code is not this code's node. So
//! a file is named for the bank's key **and the sha256 of the running
//! executable's own bytes**, and a run reads only the files its own binary
//! wrote. A file that does not decode is read as empty.
//!
//! Unset, the verb that details or looks at a campaign's places names the
//! directory itself ([`default_dir`]): `delvec detail` (the commit, `--all`
//! and `--check`) and `delvec frames --campaign` keep the bank at
//! `<campaign>/.out/reach-bank/`, so every check an author runs after the
//! first, and after the orchestrator's commit, takes the nodes those runs
//! flew. Set to the empty string, nothing is read or written. The gallery's
//! gates set it to a directory of their own scratch, so the hundred-odd probe
//! builds of one gate share what the first of them flew.
//!
//! A file is named for the running executable first (`<exe>-<bank>.bank`),
//! and a run that writes a bank removes every bank file in the directory
//! another executable wrote: no run can read them, so a rebuilt or upgraded
//! `delvec` reclaims what the last one left rather than adding to it.

use super::bank::{Run, Summary, Variant};
use super::{Death, Edge, EdgeKind, FlownKey, NodeEdges, RestKind};
use std::io::Read as _;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// The environment variable that names the directory.
pub const ENV: &str = "DELVEC_REACH_CACHE";

const MAGIC: &[u8; 8] = b"DWBANK03";

/// The nodes of one reach chunk ([`super::chunks::SIDE`]): a file holds the
/// bank chunk after chunk, and a run reads and writes it one chunk at a time.
pub(super) type Section = Vec<(FlownKey, Vec<Arc<Variant>>)>;

/// The directory the verb names when [`ENV`] is not set at all.
static DEFAULT: OnceLock<PathBuf> = OnceLock::new();

/// Name the directory the bank lives in when [`ENV`] is not set: called by
/// the verb that knows its campaign, before any bank is asked for. A second
/// call, or a call after the first bank was read, changes nothing.
pub fn default_dir(dir: PathBuf) {
    let _ = DEFAULT.set(dir);
}

/// The directory, and the running executable's sha256, when the cache is on.
fn root() -> Option<&'static (PathBuf, [u8; 32])> {
    static ROOT: OnceLock<Option<(PathBuf, [u8; 32])>> = OnceLock::new();
    ROOT.get_or_init(|| {
        let dir = match std::env::var_os(ENV) {
            Some(v) => PathBuf::from(v),
            None => DEFAULT.get()?.clone(),
        };
        if dir.as_os_str().is_empty() {
            return None;
        }
        let exe = std::fs::read(std::env::current_exe().ok()?).ok()?;
        use sha2::{Digest, Sha256};
        Some((dir, Sha256::digest(&exe).into()))
    })
    .as_ref()
}

/// The file of the bank keyed `key`, when the cache is on.
pub(super) fn file_of(key: &str) -> Option<PathBuf> {
    let (dir, exe) = root()?;
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(exe);
    h.update(key.as_bytes());
    let name: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Some(dir.join(format!("{}-{name}.bank", exe_prefix(exe))))
}

/// The part of a bank file's name that says which executable wrote it.
fn exe_prefix(exe: &[u8; 32]) -> String {
    exe[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// Remove, once a run, every bank file in the directory another executable
/// wrote. A file being written (`.bank.<pid>`) is another run's and is left.
fn reclaim_others() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        let Some((dir, exe)) = root() else {
            return;
        };
        let mine = format!("{}-", exe_prefix(exe));
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.filter_map(Result::ok) {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".bank") && !name.starts_with(&mine) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    });
}

/// Hand each chunk's nodes the file of the bank keyed `key` holds to `put`,
/// one chunk at a time (ADR-0037 §2); whether the whole file read. `false`
/// where the cache is off, the file is absent, or it does not decode — and
/// then whatever `put` was handed before the failure is not the bank's.
pub(super) fn load(key: &str, put: &mut dyn FnMut(Section)) -> bool {
    let Some(path) = file_of(key) else {
        return false;
    };
    let Ok(file) = std::fs::File::open(&path) else {
        return false;
    };
    read_from(&mut std::io::BufReader::new(file), key, put).is_some()
}

/// Write the bank keyed `key` to its file, chunk by chunk as `sections`
/// gives them, replacing it whole: a sibling written first and renamed over
/// it, so a reader never sees half of one.
pub(super) fn store(key: &str, sections: &mut dyn Iterator<Item = Section>) {
    let Some(path) = file_of(key) else {
        return;
    };
    let Some(dir) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    reclaim_others();
    let tmp = path.with_extension(format!("bank.{}", std::process::id()));
    let written = std::fs::File::create(&tmp).and_then(|f| {
        let mut w = std::io::BufWriter::new(f);
        write_to(&mut w, key, sections)?;
        use std::io::Write as _;
        w.flush()
    });
    if written.is_err() || std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

// --------------------------------------------------------------- the format

/// The writer of the format; [`super::chunks`] writes its records with it too.
#[derive(Default)]
pub(super) struct Enc(pub(super) Vec<u8>);

impl Enc {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub(super) fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn len(&mut self, n: usize) {
        self.u64(n as u64);
    }
    fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }
    pub(super) fn cell(&mut self, c: [i32; 3]) {
        for v in c {
            self.i32(v);
        }
    }
    fn box3(&mut self, b: Option<([i32; 3], [i32; 3])>) {
        match b {
            None => self.u8(0),
            Some((lo, hi)) => {
                self.u8(1);
                self.cell(lo);
                self.cell(hi);
            }
        }
    }
    fn yaw(&mut self, y: Option<i16>) {
        match y {
            None => self.u8(0),
            Some(v) => {
                self.u8(1);
                self.u16(v as u16);
            }
        }
    }
    fn death(&mut self, d: Option<Death>) {
        self.u8(match d {
            None => 0,
            Some(Death::Volume) => 1,
            Some(Death::Lava) => 2,
            Some(Death::Fall) => 3,
            Some(Death::Void) => 4,
        });
    }
    fn edge_kind(&mut self, k: EdgeKind) {
        self.u8(EdgeKind::ALL.iter().position(|x| *x == k).expect("a kind") as u8);
    }
    pub(super) fn edge(&mut self, x: &Edge) {
        self.cell(x.to);
        self.edge_kind(x.kind);
        self.yaw(x.yaw);
        self.bool(x.sprint);
    }
    pub(super) fn node_edges(&mut self, e: &NodeEdges) {
        self.u8(match e.kind {
            None => 0,
            Some(RestKind::Standing) => 1,
            Some(RestKind::Afloat) => 2,
            Some(RestKind::Submerged) => 3,
            Some(RestKind::Holding) => 4,
        });
        self.len(e.edges.len());
        for x in &e.edges {
            self.edge(x);
        }
        self.bool(e.leaves_world);
        self.bool(e.open_sea);
        self.death(e.dies);
        self.u64(e.simulated);
        self.u64(e.credited);
        self.box3(e.read);
    }
    fn summary(&mut self, s: &Summary) {
        self.len(s.edges.len());
        for ((to, kind), (yaw, sprint)) in &s.edges {
            self.cell(*to);
            self.edge_kind(*kind);
            self.yaw(*yaw);
            self.bool(*sprint);
        }
        self.bool(s.leaves_world);
        self.death(s.dies);
        self.u64(s.simulated);
        self.u64(s.credited);
        self.box3(s.grown);
    }
    fn variant(&mut self, v: &Variant) {
        self.len(v.deps.len());
        for &(i, code, x) in &v.deps {
            self.u32(i);
            self.u32(code);
            self.u32(x);
        }
        self.box3(v.head);
        self.len(v.jumps.len());
        for c in &v.jumps {
            self.cell(*c);
        }
        self.len(v.volumes.len());
        for (lo, hi) in &v.volumes {
            self.cell(*lo);
            self.cell(*hi);
        }
        self.len(v.runs.len());
        for r in &v.runs {
            self.box3(r.bbox);
            self.summary(&r.summary);
        }
        self.node_edges(&v.edges);
    }
}

/// One variant as the file writes it ([`super::bank`] spills each variant it
/// keeps in this form and reads it back by [`decode_variant`]).
pub(super) fn encode_variant(v: &Variant) -> Vec<u8> {
    let mut e = Enc::default();
    e.variant(v);
    e.0
}

/// The variant `bytes` holds, when they are exactly one [`encode_variant`].
pub(super) fn decode_variant(bytes: &[u8]) -> Option<Variant> {
    let mut d = Dec { b: bytes, at: 0 };
    let v = d.variant()?;
    (d.at == bytes.len()).then_some(v)
}

/// The file: the magic, the key, then each section as its byte length and
/// its nodes, then a length of zero — so a file cut short is refused.
fn write_to(
    w: &mut dyn std::io::Write,
    key: &str,
    sections: &mut dyn Iterator<Item = Section>,
) -> std::io::Result<()> {
    let mut e = Enc::default();
    e.0.extend_from_slice(MAGIC);
    e.len(key.len());
    e.0.extend_from_slice(key.as_bytes());
    w.write_all(&e.0)?;
    for nodes in sections {
        if nodes.is_empty() {
            continue;
        }
        let mut e = Enc::default();
        e.len(nodes.len());
        for ((c, q), vs) in &nodes {
            e.cell(*c);
            e.u16(*q);
            e.len(vs.len());
            for v in vs {
                e.variant(v);
            }
        }
        w.write_all(&(e.0.len() as u64).to_le_bytes())?;
        w.write_all(&e.0)?;
    }
    w.write_all(&0u64.to_le_bytes())
}

/// The reader of the format; [`super::chunks`] reads its records with it too.
pub(super) struct Dec<'a> {
    pub(super) b: &'a [u8],
    pub(super) at: usize,
}

impl Dec<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    pub(super) fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    /// A length, refused when it could not fit in what is left.
    fn len(&mut self) -> Option<usize> {
        let n = usize::try_from(self.u64()?).ok()?;
        (n <= self.b.len() - self.at).then_some(n)
    }
    fn bool(&mut self) -> Option<bool> {
        match self.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    pub(super) fn cell(&mut self) -> Option<[i32; 3]> {
        Some([self.i32()?, self.i32()?, self.i32()?])
    }
    fn box3(&mut self) -> Option<Option<([i32; 3], [i32; 3])>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some((self.cell()?, self.cell()?))),
            _ => None,
        }
    }
    fn yaw(&mut self) -> Option<Option<i16>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(self.u16()? as i16)),
            _ => None,
        }
    }
    fn death(&mut self) -> Option<Option<Death>> {
        Some(match self.u8()? {
            0 => None,
            1 => Some(Death::Volume),
            2 => Some(Death::Lava),
            3 => Some(Death::Fall),
            4 => Some(Death::Void),
            _ => return None,
        })
    }
    fn edge_kind(&mut self) -> Option<EdgeKind> {
        EdgeKind::ALL.get(usize::from(self.u8()?)).copied()
    }
    pub(super) fn edge(&mut self) -> Option<Edge> {
        Some(Edge {
            to: self.cell()?,
            kind: self.edge_kind()?,
            yaw: self.yaw()?,
            sprint: self.bool()?,
        })
    }
    pub(super) fn node_edges(&mut self) -> Option<NodeEdges> {
        let kind = match self.u8()? {
            0 => None,
            1 => Some(RestKind::Standing),
            2 => Some(RestKind::Afloat),
            3 => Some(RestKind::Submerged),
            4 => Some(RestKind::Holding),
            _ => return None,
        };
        let n = self.len()?;
        let mut edges = Vec::with_capacity(n);
        for _ in 0..n {
            edges.push(self.edge()?);
        }
        Some(NodeEdges {
            kind,
            edges,
            leaves_world: self.bool()?,
            open_sea: self.bool()?,
            dies: self.death()?,
            simulated: self.u64()?,
            credited: self.u64()?,
            read: self.box3()?,
        })
    }
    fn summary(&mut self) -> Option<Summary> {
        let n = self.len()?;
        let mut edges = Vec::with_capacity(n);
        for _ in 0..n {
            let to = self.cell()?;
            let kind = self.edge_kind()?;
            let yaw = self.yaw()?;
            let sprint = self.bool()?;
            edges.push(((to, kind), (yaw, sprint)));
        }
        Some(Summary {
            edges,
            leaves_world: self.bool()?,
            dies: self.death()?,
            simulated: self.u64()?,
            credited: self.u64()?,
            grown: self.box3()?,
        })
    }
    fn variant(&mut self) -> Option<Variant> {
        let n = self.len()?;
        let mut deps = Vec::with_capacity(n);
        for _ in 0..n {
            deps.push((self.u32()?, self.u32()?, self.u32()?));
        }
        let head = self.box3()?;
        let n = self.len()?;
        let mut jumps = Vec::with_capacity(n);
        for _ in 0..n {
            jumps.push(self.cell()?);
        }
        let n = self.len()?;
        let mut volumes = Vec::with_capacity(n);
        for _ in 0..n {
            volumes.push((self.cell()?, self.cell()?));
        }
        let n = self.len()?;
        let mut runs = Vec::with_capacity(n);
        for _ in 0..n {
            runs.push(Run {
                bbox: self.box3()?,
                summary: self.summary()?,
            });
        }
        Some(Variant {
            deps,
            head,
            jumps,
            volumes,
            runs,
            edges: Arc::new(self.node_edges()?),
        })
    }
}

/// Read a file [`write_to`] wrote for `key`, handing each section to `put`
/// as it is read; `None` where it does not decode whole.
fn read_from(r: &mut dyn std::io::Read, key: &str, put: &mut dyn FnMut(Section)) -> Option<()> {
    let u64_of = |r: &mut dyn std::io::Read| {
        let mut b = [0u8; 8];
        r.read_exact(&mut b).ok().map(|_| u64::from_le_bytes(b))
    };
    let mut magic = [0u8; 8];
    r.read_exact(&mut magic).ok()?;
    if &magic != MAGIC {
        return None;
    }
    let n = usize::try_from(u64_of(r)?).ok()?;
    if n != key.len() {
        return None;
    }
    let mut k = vec![0u8; n];
    r.read_exact(&mut k).ok()?;
    if k != key.as_bytes() {
        return None;
    }
    let mut buf: Vec<u8> = Vec::new();
    loop {
        let len = usize::try_from(u64_of(r)?).ok()?;
        if len == 0 {
            // The end: nothing may follow it.
            let mut rest = [0u8; 1];
            return (r.read(&mut rest).ok()? == 0).then_some(());
        }
        // Read as far as the file goes, never allocated ahead of it: a
        // damaged length is refused at the file's end.
        buf.clear();
        std::io::Read::take(&mut *r, len as u64)
            .read_to_end(&mut buf)
            .ok()?;
        if buf.len() != len {
            return None;
        }
        let mut d = Dec { b: &buf, at: 0 };
        let n = d.len()?;
        let mut out = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            let c = d.cell()?;
            let q = d.u16()?;
            let m = d.len()?;
            let mut vs = Vec::with_capacity(m.min(64));
            for _ in 0..m {
                vs.push(Arc::new(d.variant()?));
            }
            out.push(((c, q), vs));
        }
        if d.at != buf.len() {
            return None;
        }
        put(out);
    }
}

/// `sections` written as a file and read back, section by section; `None`
/// where it does not decode.
#[cfg(test)]
pub(super) fn round_trip(key: &str, sections: Vec<Section>) -> Option<Vec<Section>> {
    let mut bytes = Vec::new();
    write_to(&mut bytes, key, &mut sections.into_iter()).ok()?;
    let mut out = Vec::new();
    read_from(&mut bytes.as_slice(), key, &mut |s| out.push(s))?;
    Some(out)
}

/// The bytes `sections` take as a file, for a test that cuts one short.
#[cfg(test)]
pub(super) fn file_bytes(key: &str, sections: Vec<Section>) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_to(&mut bytes, key, &mut sections.into_iter()).expect("into memory");
    bytes
}

/// Whether `bytes` read whole as the file of `key`, and the nodes they hold.
#[cfg(test)]
pub(super) fn read_bytes(key: &str, bytes: &[u8]) -> Option<usize> {
    let mut n = 0;
    read_from(&mut &bytes[..], key, &mut |s| n += s.len())?;
    Some(n)
}
