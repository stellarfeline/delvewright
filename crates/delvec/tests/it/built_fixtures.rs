//! **The gallery's detailed points are built once per run** (spec-0126 §4.2,
//! §5). The `rust fixtures` CI job runs this one test from the instrumented
//! archive and ships the cache it fills (`common::built`'s directory) to every
//! partition, so a partition's first consumer of a point copies it instead of
//! detailing it again; in a partition this is a cache hit and costs a copy.
//! The binding is the two points' paths, printed.

use crate::common;

#[test]
fn the_gallery_points_are_built_once_per_run() {
    let tmp = std::env::temp_dir().join(format!("dw-built-fixtures-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let (open, prefabs) = common::gallery_site_plan_detailed(&tmp);
    let (solid, solid_prefabs) = common::gallery_site_plan_solid_detailed(&tmp);
    for dir in [&open, &prefabs, &solid, &solid_prefabs] {
        assert!(dir.is_dir(), "{} is not a directory", dir.display());
    }
    assert!(open.join("programs").is_dir() && solid.join("programs").is_dir());
    eprintln!(
        "built fixtures: the open point at {}, the solid point at {}; cache at {}",
        open.display(),
        solid.display(),
        common::built::cache_root().display()
    );
}
