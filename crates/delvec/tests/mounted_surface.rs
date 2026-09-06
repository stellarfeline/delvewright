//! Every mounted surface of the one binary answers through the binary.
//!
//! `delvec` mounts five crates' command lines (ADR-0023 §3), and each was
//! declared before the binary's global flags existed. A definition that
//! collides with a global — one clap id, two meanings — is not a compile error
//! and not a `Command::debug_assert` finding: it is a panic at the first parse
//! that reaches the arm, which happened in a CI step (`delvec grammar
//! coverage`) and in no local ladder. So every mounted group and every verb
//! under it is asked for `--help` here, and every arm a CI workflow runs
//! through `cargo run … -- <subcommand>` is run the way CI runs it — through
//! the binary, with the global `--json` on the far side of the verb — and a
//! panic is a failure of the test rather than of the job.
//!
//! The verb population is read off the built binary's own `--help`, never
//! listed: a verb added to a mounted crate is covered the moment it exists.

use std::process::{Command, Output};

fn delvec(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("run delvec")
}

/// A run that panicked is the defect this file exists for, whatever else it
/// did: clap's mismatch is a panic, and a panic exits 101.
fn assert_no_panic(args: &[&str], out: &Output) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.code() != Some(101)
            && !stderr.contains("panicked at")
            && !stderr.contains("Mismatch between definition and access"),
        "`delvec {}` panicked (exit {:?}):\n{stderr}",
        args.join(" "),
        out.status.code()
    );
}

/// The `Commands:` block of a `--help` page, as clap prints it.
fn verbs_of(help: &str) -> Vec<String> {
    let mut block = false;
    let mut out = Vec::new();
    for line in help.lines() {
        if line.starts_with("Commands:") {
            block = true;
            continue;
        }
        if block {
            if line.is_empty() || !line.starts_with("  ") {
                break;
            }
            let name = line.split_whitespace().next().unwrap_or("");
            if !name.is_empty() && name != "help" {
                out.push(name.to_string());
            }
        }
    }
    out
}

const GROUPS: &[&str] = &["grammar", "prefab", "schem", "harvest", "render"];

#[test]
fn every_mounted_group_and_every_verb_under_it_answers_help() {
    let mut verbs_seen = 0usize;
    for group in GROUPS {
        let out = delvec(&[group, "--help"]);
        assert_no_panic(&[group, "--help"], &out);
        assert!(
            out.status.success(),
            "`delvec {group} --help` exited {:?}",
            out.status.code()
        );
        let help = String::from_utf8_lossy(&out.stdout).into_owned();
        for verb in verbs_of(&help) {
            let args = [group, verb.as_str(), "--help"];
            let out = delvec(&args);
            assert_no_panic(&args, &out);
            assert!(
                out.status.success(),
                "`delvec {group} {verb} --help` exited {:?}",
                out.status.code()
            );
            verbs_seen += 1;
        }
    }
    // `harvest` has no verbs; the four other groups carry nineteen between them.
    assert!(
        verbs_seen >= 19,
        "only {verbs_seen} verb(s) answered — the walk is not the tree"
    );
}

/// The arms CI runs through the binary (`.github/workflows/ci.yml`, every
/// `cargo run … -- <subcommand>` step) and the arms the skill runs, each with
/// the global `--json` placed after the verb — the position that reaches the
/// mounted crate's own field reader with the global already in its matches.
#[test]
fn every_arm_a_workflow_runs_parses_through_the_binary() {
    let runs: &[(&[&str], &[i32])] = &[
        // ci.yml `rust` job: grammar demonstration coverage.
        (&["grammar", "coverage"], &[0, 4]),
        (&["grammar", "coverage", "--json"], &[0, 4]),
        (&["grammar", "list"], &[0]),
        (&["grammar", "list", "--json"], &[0]),
        (&["grammar", "show", "--program", "store-room"], &[0]),
        (&["grammar", "check", "--program", "store-room"], &[0]),
        // A missing input is a refusal by exit code, never a panic.
        (&["prefab", "audit", "definitely-not-here.nbt"], &[2]),
        (
            &["prefab", "audit", "definitely-not-here.nbt", "--json"],
            &[2],
        ),
        (&["prefab", "lighting", "definitely-not-here.nbt"], &[2]),
        (
            &[
                "schem",
                "convert",
                "definitely-not-here.schem",
                "-o",
                "out.nbt",
            ],
            &[2],
        ),
        (&["schem", "convert", "--help"], &[0]),
        (
            &[
                "harvest",
                "definitely-not-here.log",
                "definitely-not-here.json",
            ],
            &[1],
        ),
        // gpu-probe.yml: the fidelity gate. On a runner with no client jar and
        // no adapter it refuses with the render tier's own code (5); on a
        // workstation whose texture ladder falls through to a real jar beside
        // a real adapter it renders and passes (0). Either is a parse that
        // reached the arm, which is what this row is for.
        (
            &[
                "render",
                "fidelity-gate",
                "--textures",
                "definitely-not-here.jar",
            ],
            &[0, 5],
        ),
        (
            &[
                "render",
                "fidelity-gate",
                "--textures",
                "definitely-not-here.jar",
                "--json",
            ],
            &[0, 5],
        ),
        (
            &["render", "piece", "definitely-not-here.nbt", "-o", "shots"],
            &[2, 5],
        ),
    ];
    for (args, exits) in runs {
        let out = delvec(args);
        assert_no_panic(args, &out);
        assert!(
            exits.contains(&out.status.code().unwrap_or(-1)),
            "`delvec {}` exited {:?}, expected one of {exits:?}:\n{}",
            args.join(" "),
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
