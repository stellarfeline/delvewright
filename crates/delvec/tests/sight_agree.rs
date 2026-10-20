//! **One "in sight" rule** (CLAUDE.md, *one question, one implementation*):
//! whether an eye sees a box past the cells that stop a line is
//! `framing::in_sight`, and every consumer of that question gives its answer.
//! The consumers: the camera's sight (`framing::sight`, read by `DW1033` and by
//! the snapshot manifest's `occluded`) and an act's sight from where the party
//! stands (`promise::act_unfound`, `DW0863`). The same generated cases are
//! asked of each, red on any difference.

use std::collections::BTreeSet;

use delvec::compiler::framing::{CellBox, Pose, in_sight, sight};
use delvec::compiler::nav::World;
use delvec::compiler::promise::act_unfound;

/// A deterministic generator (ADR-0006): a 64-bit LCG, seeded.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> i32 {
        (self.next() % n) as i32
    }
}

#[test]
fn every_consumer_of_in_sight_answers_alike() {
    let mut rng = Lcg(0x5167_4a6e);
    let cases = 2000;
    let (mut seen, mut hidden) = (0usize, 0usize);
    let mut differ = Vec::new();
    for case in 0..cases {
        // A 12 × 6 × 12 room with a floor at y = 0, some of it filled.
        let mut solid = BTreeSet::new();
        for x in 0..12 {
            for z in 0..12 {
                solid.insert([x, 0, z]);
            }
        }
        let density = 4 + rng.below(20) as u64;
        for x in 0..12 {
            for y in 1..6 {
                for z in 0..12 {
                    if (rng.next() % 100) < density {
                        solid.insert([x, y, z]);
                    }
                }
            }
        }
        let from = [rng.below(12), 1, rng.below(12)];
        solid.remove(&from);
        solid.remove(&[from[0], 2, from[2]]);
        let target = [rng.below(12), 1 + rng.below(3), rng.below(12)];
        let body = rng.below(2) == 1;
        if target == from {
            continue;
        }
        let world = World::from_solid_cells(solid);
        let blocks = |c: [i32; 3]| world.blocks_camera(c);
        let eye = [from[0] as f64 + 0.5, 1.0 + 1.62, from[2] as f64 + 0.5];
        let sight_box = if body {
            CellBox::spanning(target, [target[0], target[1] + 1, target[2]])
        } else {
            CellBox::cell(target)
        };
        let rule = in_sight(eye, &sight_box, &blocks).seen();
        let camera = !sight(
            &Pose {
                pos: eye,
                yaw: 0.0,
                pitch: 0.0,
            },
            70.0,
            854.0 / 480.0,
            sight_box.centre(),
            &sight_box,
            &blocks,
        )
        .occluded();
        let act = !act_unfound(&world, from, target, body, None)
            .iter()
            .any(|why| why.contains("hidden"));
        if rule {
            seen += 1;
        } else {
            hidden += 1;
        }
        if camera != rule {
            differ.push(format!(
                "case {case}: framing::sight says {camera}, the rule {rule}"
            ));
        }
        if act != rule {
            differ.push(format!("case {case}: DW0863 says {act}, the rule {rule}"));
        }
    }
    println!(
        "in-sight agreement: {} case(s), {seen} seen and {hidden} hidden; 2 consumer(s) \
         (framing::sight, promise::act_unfound) each asked every case",
        seen + hidden
    );
    assert!(seen > 0 && hidden > 0, "the cases exercise both answers");
    assert!(
        differ.is_empty(),
        "{} difference(s):\n{}",
        differ.len(),
        differ.join("\n")
    );
}
