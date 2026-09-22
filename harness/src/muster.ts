// The MUSTER: what the delve declared a wave to be, checked against the bodies
// the server actually seated.
//
// ## What a combat step is for
//
// The ladder does not fight. At a combat encounter it verifies MECHANISM — the
// wave spawned as declared, the kill wiring fired, the re-seat re-seated, dying
// is safe — and nothing about whether the fight can be won. Whether a delve is
// too hard or too easy is the owner's playtest hour, not a number a bot can
// produce.
//
// Of those, "the wave spawned as declared" is the one nothing looked at. A wave
// mob's `max_health`, `attack_damage`, `equipment` and `name` go into a single
// `summon` line and were never read back. The pinned server silently drops NBT it
// does not recognise — it has done so twice here, on `HandItems` and on the
// legacy `PatrolTarget` compound — so a declaration that never reaches the body
// produces a delve that boots, plays, and is not the document.
//
// The compiler emits the probe (`crates/delvec/src/compiler/muster.rs`) and this
// module reads it. Every number crosses the anchored chat channel as a
// fixed-point integer, exactly like the census; every identity fact (a name, an
// item in a slot) is a bit the server itself set by answering `execute if data`,
// so no player-visible text is ever parsed.
//
// Pure: parsing, comparison and verdicts. The executor supplies the bot.

import type { MusterPlan, MusterProfile, MusterType } from "./combat.ts";

/** A signed integer field as it appears on the wire. */
const INT = "(-?[0-9]+)";
const WAVE = "(wave\\/[a-z0-9]+(?:-[a-z0-9]+)*)";
const CAMPAIGN = "([a-z0-9]+(?:-[a-z0-9]+)*)";

const SUMMARY_RE = new RegExp(`^\\[dw:muster ${CAMPAIGN} ${WAVE} ${INT} ${INT} ${INT}\\]$`);
const BODY_RE = new RegExp(
  `^\\[dw:musterbody ${CAMPAIGN} ${WAVE} ${INT} ${INT} ${INT} ${INT} ${INT} ${INT} ${INT} ${INT} ${INT} ${INT}\\]$`,
);

/** The line that closes one muster of one wave. */
export interface MusterSummary {
  readonly campaignId: string;
  readonly wave: string;
  /** Server-side counter; strictly increasing per muster. */
  readonly seq: number;
  /** Bodies of a DECLARED entity kind that the probe read. */
  readonly counted: number;
  /** Bodies carrying the wave's tag at all. Above `counted` means the wave
   * seated something of a kind it never declared. */
  readonly tagged: number;
}

/** One live body's reading. Attributes are raw fixed-point, at the plan's scale. */
export interface MusterBody {
  readonly campaignId: string;
  readonly wave: string;
  readonly seq: number;
  /** Index into the plan's `types`. */
  readonly typeIndex: number;
  /** Which of that kind's declared identity facts hold on this body. */
  readonly mask: number;
  readonly maxHealth: number;
  readonly armor: number;
  readonly armorToughness: number;
  readonly movementSpeed: number;
  /** The plan's `unread` sentinel where the probe did not ask. */
  readonly attackDamage: number;
  readonly followRange: number;
  /**
   * What the body actually swings with, weapon modifiers included — telemetry,
   * compared against nothing. A Guard declared `attack_damage: 6.0` hits for 11.0
   * with its iron sword, and until this reading no artifact said so.
   */
  readonly attackDamageEffective: number;
}

/** Parse one chat line as a muster summary, or `undefined`. Whole-line, strict. */
export function parseMusterSummary(line: string): MusterSummary | undefined {
  const m = SUMMARY_RE.exec(line);
  if (!m) return undefined;
  return {
    campaignId: m[1]!,
    wave: m[2]!,
    seq: Number(m[3]),
    counted: Number(m[4]),
    tagged: Number(m[5]),
  };
}

/** Parse one chat line as a muster body reading, or `undefined`. */
export function parseMusterBody(line: string): MusterBody | undefined {
  const m = BODY_RE.exec(line);
  if (!m) return undefined;
  return {
    campaignId: m[1]!,
    wave: m[2]!,
    seq: Number(m[3]),
    typeIndex: Number(m[4]),
    mask: Number(m[5]),
    maxHealth: Number(m[6]),
    armor: Number(m[7]),
    armorToughness: Number(m[8]),
    movementSpeed: Number(m[9]),
    attackDamage: Number(m[10]),
    followRange: Number(m[11]),
    attackDamageEffective: Number(m[12]),
  };
}

/** What one muster established, and what it could not. */
export interface MusterVerdict {
  readonly wave: string;
  /** Declared facts this probe put a question to — the binding count. Zero is a
   * finding in its own right unless the wave declares nothing. */
  readonly checked: number;
  /** Bodies the probe read. */
  readonly read: number;
  /** Bodies the plan says the wave seats. */
  readonly declared: number;
  /** Bodies that matched a declared stack in every checked respect. */
  readonly matched: number;
  /** Everything that did not hold, in the declaration's own terms. */
  readonly findings: readonly string[];
}

/** One declared body slot awaiting an observation. */
interface Slot {
  readonly profile: MusterProfile;
  readonly type: MusterType;
  taken: boolean;
}

/**
 * Compare what the server seated against what the campaign declared.
 *
 * Matching is by MULTISET, never by position: a live body carries no memory of
 * which declared stack it came from, and a wave may seat two stacks of one kind
 * that differ only in name (`wave/drowned-choir`). So each observation claims an
 * unclaimed declared slot it satisfies exactly; what is left over on either side
 * is the finding, diagnosed against the nearest slot of the same kind so the
 * reader is told WHICH declared fact is missing rather than that something is.
 */
export function verifyMuster(
  plan: MusterPlan,
  summary: MusterSummary,
  bodies: readonly MusterBody[],
): MusterVerdict {
  const findings: string[] = [];
  const slots: Slot[] = [];
  for (const profile of plan.profiles) {
    const type = plan.types[profile.typeIndex];
    if (!type) continue;
    for (let i = 0; i < profile.count; i += 1) slots.push({ profile, type, taken: false });
  }

  // Nothing standing is not the same fact as a body that is wrong, and it must
  // not be reported as N missing stacks. The wave may legitimately be gone — the
  // world felled it, or a run-back's cohort was cleared — and what is true is
  // only that the probe had nothing to read.
  if (summary.tagged === 0) {
    return {
      wave: summary.wave,
      checked: plan.checked,
      read: 0,
      declared: plan.bodies,
      matched: 0,
      findings: [
        `nothing of this wave was standing when the muster ran, so none of its ` +
          `${plan.checked} declared fact(s) could be checked against a body ` +
          `(${plan.bodies} declared: ${plan.profiles.map((p) => p.label).join("; ")})`,
      ],
    };
  }
  if (summary.tagged !== summary.counted) {
    findings.push(
      `${plan.probe}: ${summary.tagged} body/bodies carry the wave's tag but only ` +
        `${summary.counted} are of a kind the wave declares — ${summary.tagged - summary.counted} ` +
        `body/bodies of an undeclared entity kind are standing in this encounter`,
    );
  }
  if (bodies.length !== summary.counted) {
    findings.push(
      `${plan.probe}: the muster summary counted ${summary.counted} body/bodies but ` +
        `${bodies.length} reading(s) reached the harness — the probe's own lines and its total ` +
        `disagree, so neither is evidence`,
    );
  }
  if (bodies.length !== slots.length) {
    findings.push(
      `wave seating: the campaign declares ${slots.length} body/bodies ` +
        `(${plan.profiles.map((p) => p.label).join("; ")}) and the server seated ${bodies.length}`,
    );
  }

  let matched = 0;
  const unmatched: MusterBody[] = [];
  for (const body of bodies) {
    const slot = slots.find((s) => !s.taken && satisfies(plan, s, body).length === 0);
    if (slot) {
      slot.taken = true;
      matched += 1;
    } else {
      unmatched.push(body);
    }
  }
  for (const body of unmatched) {
    const candidates = slots.filter((s) => !s.taken && s.profile.typeIndex === body.typeIndex);
    const best = candidates
      .map((s) => ({ slot: s, gaps: satisfies(plan, s, body) }))
      .sort((a, b) => a.gaps.length - b.gaps.length)[0];
    if (!best) {
      const kind = plan.types[body.typeIndex]?.entity ?? `type ${body.typeIndex}`;
      findings.push(
        `a live ${kind} stands in ${plan.probe.split(":").pop()} that no declared stack accounts ` +
          `for (identity mask ${body.mask})`,
      );
      continue;
    }
    findings.push(
      `${best.slot.profile.label}: the body the server seated is not what the campaign ` +
        `declared — ${best.gaps.join("; ")}`,
    );
    best.slot.taken = true;
  }
  const missing = slots.filter((s) => !s.taken);
  for (const s of missing) {
    findings.push(`${s.profile.label}: declared but no body answering to it was read`);
  }

  return {
    wave: summary.wave,
    checked: plan.checked,
    read: bodies.length,
    declared: plan.bodies,
    matched,
    findings,
  };
}

/** Every way `body` fails to be what `slot` declares; empty when it is. */
function satisfies(plan: MusterPlan, slot: Slot, body: MusterBody): string[] {
  const gaps: string[] = [];
  if (slot.profile.typeIndex !== body.typeIndex) {
    gaps.push(
      `it is a ${plan.types[body.typeIndex]?.entity ?? `type ${body.typeIndex}`}, not a ` +
        `${slot.type.entity}`,
    );
    return gaps;
  }
  for (let bit = 0; bit < slot.type.facts.length; bit += 1) {
    const wanted = (slot.profile.mask & (1 << bit)) !== 0;
    const held = (body.mask & (1 << bit)) !== 0;
    if (wanted && !held) gaps.push(`the body does not carry \`${slot.type.facts[bit]}\``);
    if (!wanted && held) gaps.push(`the body carries \`${slot.type.facts[bit]}\`, undeclared here`);
  }
  const at = (declared: number | undefined, read: number, name: string): void => {
    if (declared === undefined) return;
    if (read === plan.unread) {
      gaps.push(`\`${name}\` is declared ${declared} but the probe did not read it`);
      return;
    }
    const want = Math.round(declared * plan.scale);
    if (read !== want) {
      gaps.push(
        `\`${name}\` is declared ${declared} and the body's own attribute reads ` +
          `${read / plan.scale}`,
      );
    }
  };
  at(slot.profile.maxHealth, body.maxHealth, "max_health");
  at(slot.profile.attackDamage, body.attackDamage, "attack_damage");
  at(slot.profile.movementSpeed, body.movementSpeed, "movement_speed");
  at(slot.profile.followRange, body.followRange, "follow_range");
  const floor = (least: number, read: number, name: string, what: string): void => {
    if (least <= 0) return;
    if (read < Math.round(least * plan.scale)) {
      gaps.push(
        `the declared ${what} is worth ${least} \`${name}\` and the body's own attribute reads ` +
          `${read / plan.scale} — the gear did not reach it`,
      );
    }
  };
  floor(slot.profile.armorAtLeast, body.armor, "armor", "equipment");
  floor(slot.profile.armorToughnessAtLeast, body.armorToughness, "armor_toughness", "equipment");
  return gaps;
}
