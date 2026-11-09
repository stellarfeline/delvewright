// What a 1.21.11 warden does when a maze's players try to steer it — measured
// on a THROWAWAY pinned server, never the shared stack (the pattern of
// `warden-probe.sh`). The questions a co-op maze level asks, each answered by
// running it rather than by the wiki:
//
//   T1  a projectile landing near a roaming warden: from how far does it turn,
//       and for how long does it hold the spot;
//   T2  a warden already hunting a body: does a projectile — sourceless, then
//       thrown by a player standing on a platform above — still turn it, and
//       what the thrower earns for it;
//   T3  its pursuit down a 1-wide and a 2-wide corridor;
//   T4  a sneaking player beside it, against a walking one;
//   T5  a bell rung by a block state, by redstone power, and `playsound` alone;
//   T6  whether a command summon can play the emerge-from-the-ground animation;
//   T7  the wither's spawn charge-up (`Invul`): its length, what the body does
//       and cannot do while it runs, the blast that ends it, and whether a NoAI
//       body plays it at all;
//   T8  the emerge window: whether an emerging warden hears a walker, takes a
//       blow, and keeps a seeded anger through to the hunt;
//   T9  whether a body summoned and removed in one tick is ever seen by a client
//       — the shape an `ambushes[]` puppet takes under `unleash-actor`.
//
// One mineflayer bot is the "player" where a player is the question (T2, T4, T6,
// T7, T8, T9: it reads the warden's synced pose, the wither's boss bar and every
// entity spawn off its own client). Everything else is read back over rcon
// through the repo's one rejection rule (`tools/lib/rcon.mjs`).
// The written records are `docs/reference/warden-behaviour.md` (T1–T6, T8) and
// `docs/reference/native-entrance.md` (T7, T9).
//
//   docker run -d --name dw-<round>-warden --cpus=2 --memory=2g -e EULA=TRUE \
//     -e TYPE=VANILLA -e VERSION=1.21.11 -e DIFFICULTY=easy \
//     -e LEVEL_TYPE=minecraft:flat -e SPAWN_MONSTERS=false -e SPAWN_ANIMALS=false \
//     -e ONLINE_MODE=FALSE -e ENABLE_RCON=TRUE -e RCON_PASSWORD=<pw> \
//     ghcr.io/stellarfeline/delvewright-base@sha256:<versions.toml [images.base].digest>
//   node validation/warden-maze-probe.mjs dw-<round>-warden [only=T1,T5]
//
// The bot joins the container's own address (no host port is published), so the
// script runs on the Docker host. mineflayer is the harness's pinned copy,
// resolved from `harness/` (run `npm ci` there first).
//
// Output: one TSV line per reading — trial, elapsed ms, and the fields the trial
// reads.
import { createRequire } from 'node:module'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { rconChannel } from '../tools/lib/rcon.mjs'

const require = createRequire(new URL('../harness/package.json', import.meta.url))
const mineflayer = require('mineflayer')
const { Vec3 } = require('vec3')
const execFileP = promisify(execFile)

const container = process.argv[2]
if (!container) {
  console.error('usage: node validation/warden-maze-probe.mjs <container> [only=T1,T2,...]')
  process.exit(2)
}
const onlyArg = process.argv.slice(3).find((a) => a.startsWith('only='))
const only = onlyArg ? new Set(onlyArg.slice(5).split(',')) : null
const wants = (t) => !only || only.has(t)

const rcon = rconChannel(container)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const now = () => Date.now()

// ---- arena ------------------------------------------------------------------
// A stone floor at y=100 in a superflat world (its own surface is at y=-60, so
// everything above the pad is open air and open sky). The warden stands at W.
const W = { x: 200, y: 101, z: 200 }
const FLOOR_Y = 100
const WARDEN_SEL = '@e[type=minecraft:warden,limit=1]'
const BOT = 'dw-maze-probe'

const KEEP_AWAKE = '"minecraft:dig_cooldown":{value:{},ttl:1200L}'
const awakeNbt = (extraMemories = '', extraTop = '') =>
  `{PersistenceRequired:1b,Brain:{memories:{${KEEP_AWAKE}${extraMemories ? ',' + extraMemories : ''}}}${extraTop ? ',' + extraTop : ''}}`

function valueOf(reply) {
  const m = reply.match(/entity data: (.*)$/s)
  return m ? m[1].trim() : null
}
async function wardenGet(path) {
  return valueOf(await rcon.probe(`data get entity ${WARDEN_SEL} ${path}`))
}
async function entityGet(sel, path) {
  return valueOf(await rcon.probe(`data get entity ${sel} ${path}`))
}
function parsePos(s) {
  if (!s) return null
  const m = s.match(/\[([-\d.]+)d, ([-\d.]+)d, ([-\d.]+)d\]/)
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null
}
const dist2 = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2])
const f1 = (n) => (n == null ? '-' : n.toFixed(1))

let trialName = ''
let t0 = 0
function row(...fields) {
  console.log([trialName, `${now() - t0}ms`, ...fields].join('\t'))
}
function begin(name, note) {
  trialName = name
  t0 = now()
  console.log(`#TRIAL\t${name}\t${note}`)
}

async function clearBodies() {
  await rcon.probe('kill @e[type=minecraft:warden]')
  await rcon.probe('kill @e[type=minecraft:zombie]')
  await rcon.probe('kill @e[type=minecraft:snowball]')
  await rcon.probe('kill @e[type=minecraft:egg]')
  await rcon.probe('kill @e[type=minecraft:item]')
  // A thrown egg hatches a chicken one time in eight, and a chicken's steps are
  // vibrations with a UUID of their own: the first run read a stranger's anger
  // climbing to 118 beside a sneaking bot and it was a chicken.
  await rcon.probe('kill @e[type=minecraft:chicken]')
  await rcon.probe('kill @e[type=minecraft:experience_orb]')
  await rcon.probe('kill @e[type=minecraft:wither]')
  await rcon.probe('kill @e[type=minecraft:wither_skull]')
  await rcon.probe('kill @e[type=minecraft:villager]')
  await rcon.probe('kill @e[tag=dw_blink]')
  await sleep(600)
}
async function summonWarden(at, nbt) {
  await rcon.run(`summon minecraft:warden ${at.x} ${at.y} ${at.z}${nbt ? ' ' + nbt : ''}`)
}
async function summonVictim(at) {
  await rcon.probe('kill @e[tag=dw_victim]')
  await rcon.run(
    // Not `Invulnerable`: the warden's target filter drops an invulnerable body
    // (the first run's seeded anger at one read back as `[]` on the next tick).
    // Resistance V makes it unkillable while still a valid target.
    `summon minecraft:zombie ${at.x} ${at.y} ${at.z} {NoAI:1b,Silent:1b,PersistenceRequired:1b,Tags:["dw_victim"],active_effects:[{id:"minecraft:resistance",amplifier:4b,duration:-1,show_particles:0b}]}`,
  )
  await sleep(300)
  const uuid = await entityGet('@e[tag=dw_victim,limit=1]', 'UUID')
  if (!uuid) throw new Error('victim has no UUID readback')
  return uuid
}
const angerAt = (uuid, anger) => `anger:{suspects:[{anger:${anger},uuid:${uuid}}]}`

// A reading of the warden: where it is, what it is angry at, what disturbed it,
// and what it is attacking. `anger` is the raw suspects list (uuid + anger).
async function reading() {
  const pos = parsePos(await wardenGet('Pos'))
  const anger = await wardenGet('anger.suspects')
  // `disturbance_location` is not saved to NBT on this version and always reads
  // back null; it is kept in the reading so the fact stays visible in the TSV.
  const dist = await wardenGet('Brain.memories."minecraft:disturbance_location"')
  const target = await wardenGet('Brain.memories."minecraft:attack_target"')
  const roar = await wardenGet('Brain.memories."minecraft:roar_target"')
  return { pos, anger, dist, target, roar }
}

// ---- the bot ----------------------------------------------------------------
async function containerIp() {
  const { stdout } = await execFileP('docker', [
    'inspect',
    '-f',
    '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}',
    container,
  ])
  const ip = stdout.trim()
  if (!ip) throw new Error('container has no IP address')
  return ip
}
async function joinBot() {
  const host = await containerIp()
  const bot = mineflayer.createBot({ host, port: 25565, version: '1.21.11', username: BOT, auth: 'offline' })
  await new Promise((resolve, reject) => {
    bot.once('spawn', resolve)
    bot.once('kicked', (r) => reject(new Error(`kicked: ${JSON.stringify(r)}`)))
    bot.once('error', reject)
  })
  bot.on('error', (e) => console.error(`bot error: ${e?.message ?? e}`))
  bot.on('death', () => console.log(`#BOT\tdied`))
  return bot
}
async function park(bot) {
  // 40 blocks from W: past the 16-block vibration reach and the sniff.
  await rcon.run(`tp ${BOT} ${W.x} ${W.y} ${W.z - 40}`)
  bot.clearControlStates()
  await sleep(800)
}
async function healBot() {
  await rcon.probe(`effect give ${BOT} minecraft:instant_health 1 10 true`)
  await rcon.probe(`effect give ${BOT} minecraft:saturation 1 10 true`)
}
function wardenEntity(bot) {
  return Object.values(bot.entities).find((e) => e.name === 'warden')
}

// ---- trials -----------------------------------------------------------------

// T1: a sourceless snowball lands d blocks from a roaming warden.
async function t1(bot) {
  await park(bot)
  // `disturbance_location` is not a saved memory on this version (the first run
  // read it back as null while the warden walked to the spot), so the reading is
  // the warden's own feet: heard = it reaches within 2 blocks of the landing cell.
  for (const d of [0, 8, 12, 16, 18, 20, 24]) {
    await clearBodies()
    const control = d === 0
    begin('T1-projectile-reach', control ? 'control: no snowball; the roaming warden is read for 15s' : `snowball lands ${d} blocks east of a roaming warden`)
    await summonWarden(W, awakeNbt())
    await sleep(3000)
    const before = await reading()
    const landing = [W.x + (control ? 16 : d), FLOOR_Y + 1, W.z]
    const d0 = before.pos ? dist2(before.pos, landing) : null
    row(`d=${d}`, 'before', `pos=${before.pos?.map(f1)}`, `wardenToLanding=${f1(d0)}`, `anger=${before.anger}`)
    if (!control) await rcon.run(`summon minecraft:snowball ${landing[0]} ${FLOOR_Y + 4} ${landing[2]}`)
    const tLand = now()
    let reachedMs = null
    let leftMs = null
    let nearest = Infinity
    while (now() - tLand < 15_000) {
      const r = await reading()
      const dl = r.pos ? dist2(r.pos, landing) : null
      if (dl != null) {
        nearest = Math.min(nearest, dl)
        if (dl <= 2 && reachedMs == null) reachedMs = now() - tLand
        if (reachedMs != null && leftMs == null && dl > 2) leftMs = now() - tLand
      }
      row(`d=${d}`, `pos=${r.pos?.map(f1)}`, `toLanding=${f1(dl)}`, `anger=${r.anger}`)
      await sleep(500)
    }
    row(`d=${d}`, 'summary', `wardenToLandingAtLanding=${f1(d0)}`, `reached=${reachedMs != null}`, `reachedMs=${reachedMs}`, `leftMs=${leftMs}`, `nearestToLanding=${f1(nearest)}`)
  }
}

// T2: a warden hunting a body, then a projectile — sourceless, then thrown by a
// player 10 blocks away and 11 blocks up.
async function t2(bot) {
  await park(bot)
  await clearBodies()
  begin('T2-hunt-vs-sourceless', 'warden angry 150 at an invulnerable NoAI zombie 15 south; snowball lands 12 east')
  const victim = { x: W.x, y: W.y, z: W.z + 15 }
  const uuid = await summonVictim(victim)
  await summonWarden(W, awakeNbt('', angerAt(uuid, 150)))
  await sleep(2500)
  let r = await reading()
  row('hunting', `pos=${r.pos?.map(f1)}`, `target=${r.target}`, `anger=${r.anger}`)
  await rcon.run(`summon minecraft:snowball ${W.x + 12} ${FLOOR_Y + 4} ${W.z}`)
  const tLand = now()
  while (now() - tLand < 12_000) {
    r = await reading()
    const v = [victim.x, victim.y, victim.z]
    row(`pos=${r.pos?.map(f1)}`, `toVictim=${r.pos ? f1(dist2(r.pos, v)) : '-'}`, `toLanding=${r.pos ? f1(dist2(r.pos, [W.x + 12, 0, W.z])) : '-'}`, `dist=${r.dist}`, `target=${r.target}`, `anger=${r.anger}`)
    await sleep(500)
  }

  await clearBodies()
  begin('T2-hunt-vs-thrown-egg', 'same hunt; the bot throws eggs from a platform 10 east and 11 up')
  // The platform: 3x3 of stone at y=111, the bot on it.
  const P = { x: W.x + 10, y: 112, z: W.z }
  await rcon.run(`fill ${P.x - 1} 111 ${P.z - 1} ${P.x + 1} 111 ${P.z + 1} minecraft:stone`)
  await rcon.run(`tp ${BOT} ${P.x} ${P.y} ${P.z}`)
  await rcon.run(`clear ${BOT}`)
  await rcon.run(`give ${BOT} minecraft:egg 16`)
  await healBot()
  await sleep(800)
  const uuid2 = await summonVictim(victim)
  await summonWarden(W, awakeNbt('', angerAt(uuid2, 150)))
  await sleep(2500)
  r = await reading()
  row('hunting', `pos=${r.pos?.map(f1)}`, `target=${r.target}`, `anger=${r.anger}`, `botHealth=${bot.health}`)
  const egg = bot.inventory.items().find((i) => i.name === 'egg')
  if (!egg) throw new Error('bot holds no egg')
  await bot.equip(egg, 'hand')
  const tThrow = now()
  for (let i = 0; i < 6; i++) {
    // Aim at the warden's feet so the egg lands beside it.
    const w = wardenEntity(bot)
    const aim = w ? w.position.offset(0, 0.5, 0) : new Vec3(W.x + 0.5, W.y, W.z + 0.5)
    await bot.lookAt(aim, true)
    bot.activateItem()
    await sleep(150)
    bot.deactivateItem()
    await sleep(850)
    r = await reading()
    row(`egg=${i + 1}`, `pos=${r.pos?.map(f1)}`, `toVictim=${r.pos ? f1(dist2(r.pos, [victim.x, 0, victim.z])) : '-'}`, `target=${r.target}`, `roar=${r.roar}`, `anger=${r.anger}`, `botHealth=${bot.health}`)
  }
  while (now() - tThrow < 25_000) {
    r = await reading()
    row(`pos=${r.pos?.map(f1)}`, `toVictim=${r.pos ? f1(dist2(r.pos, [victim.x, 0, victim.z])) : '-'}`, `toBot=${r.pos ? f1(dist2(r.pos, [P.x, 0, P.z])) : '-'}`, `target=${r.target}`, `roar=${r.roar}`, `anger=${r.anger}`, `botHealth=${bot.health}`)
    await sleep(1000)
  }
  row('botUuid', bot.player?.uuid ?? bot.entity?.uuid)
  await rcon.run(`fill ${P.x - 1} 111 ${P.z - 1} ${P.x + 1} 111 ${P.z + 1} minecraft:air`)
  await healBot()
}

// T3: pursuit down a corridor of width 1 and 2, walls 4 high, open top.
async function t3(bot) {
  await park(bot)
  for (const width of [1, 2]) {
    await clearBodies()
    begin(`T3-corridor-${width}w`, `warden angry 150 at a zombie 18 blocks down a ${width}-wide, 4-high, open-top corridor`)
    const x0 = W.x + 20 + width * 6
    const z0 = W.z
    const len = 18
    // Walls either side of the lane x0..x0+width-1.
    await rcon.run(`fill ${x0 - 1} ${FLOOR_Y + 1} ${z0} ${x0 - 1} ${FLOOR_Y + 4} ${z0 + len} minecraft:stone`)
    await rcon.run(`fill ${x0 + width} ${FLOOR_Y + 1} ${z0} ${x0 + width} ${FLOOR_Y + 4} ${z0 + len} minecraft:stone`)
    // Closed at the far end behind the victim, so the only way to it is the lane.
    await rcon.run(`fill ${x0 - 1} ${FLOOR_Y + 1} ${z0 + len + 1} ${x0 + width} ${FLOOR_Y + 4} ${z0 + len + 1} minecraft:stone`)
    const victim = { x: x0, y: W.y, z: z0 + len }
    const start = { x: x0, y: W.y, z: z0 + 1 }
    const uuid = await summonVictim(victim)
    await summonWarden(start, awakeNbt('', angerAt(uuid, 150)))
    const tStart = now()
    let arrived = null
    while (now() - tStart < 30_000) {
      const r = await reading()
      const d = r.pos ? dist2(r.pos, [victim.x + 0.5, 0, victim.z + 0.5]) : null
      row(`pos=${r.pos?.map(f1)}`, `toVictim=${f1(d)}`, `target=${r.target}`)
      if (d != null && d <= 2.0 && arrived == null) {
        arrived = now() - tStart
        break
      }
      await sleep(500)
    }
    row('summary', `arrivedMs=${arrived}`, `lenBlocks=${len - 1}`)
    await rcon.run(`fill ${x0 - 1} ${FLOOR_Y + 1} ${z0} ${x0 + width} ${FLOOR_Y + 4} ${z0 + len + 1} minecraft:air`)
  }
}

// T4: a sneaking player against a walking one, 10 blocks from a calm warden;
// then a sneaking player standing 4 blocks from it (the sniff).
async function t4(bot) {
  const runLeg = async (name, sneak, startZ, seconds, walk) => {
    await park(bot)
    await clearBodies()
    begin(name, `bot ${sneak ? 'sneaks' : 'walks'} ${walk ? 'to and fro' : 'and stands'} ${W.z - startZ} blocks north of a calm warden for ${seconds}s`)
    await summonWarden(W, awakeNbt())
    await rcon.run(`tp ${BOT} ${W.x} ${W.y} ${startZ}`)
    await sleep(1500)
    bot.setControlState('sneak', sneak)
    const tStart = now()
    let dir = 1
    while (now() - tStart < seconds * 1000) {
      if (walk) {
        const target = new Vec3(W.x + (dir > 0 ? 6 : -6) + 0.5, W.y, startZ + 0.5)
        await bot.lookAt(target.offset(0, 1.6, 0), true)
        bot.setControlState('forward', true)
        await sleep(1500)
        bot.setControlState('forward', false)
        dir = -dir
      } else {
        await sleep(1500)
      }
      const r = await reading()
      row(`botPos=${[bot.entity.position.x, bot.entity.position.z].map(f1)}`, `pos=${r.pos?.map(f1)}`, `anger=${r.anger}`, `dist=${r.dist}`, `target=${r.target}`, `botHealth=${bot.health}`)
    }
    bot.clearControlStates()
    const r = await reading()
    row('summary', `anger=${r.anger}`, `target=${r.target}`, `botHealth=${bot.health}`)
    await healBot()
  }
  await runLeg('T4-walk-at-10', false, W.z - 10, 15, true)
  await runLeg('T4-sneak-at-10', true, W.z - 10, 15, true)
  await runLeg('T4-sneak-stand-at-4', true, W.z - 4, 20, false)
}

// T5: a bell at distance d from a calm warden: a powered block state, a redstone
// block beside it, and `playsound` alone.
async function t5(bot) {
  await park(bot)
  const probeBell = async (label, d, act) => {
    await clearBodies()
    begin(`T5-bell-${label}`, `bell ${d} blocks east of a calm warden; ${label}`)
    const B = { x: W.x + d, y: W.y, z: W.z }
    await rcon.run(`setblock ${B.x} ${B.y} ${B.z} minecraft:bell[attachment=floor,facing=north]`)
    await summonWarden(W, awakeNbt())
    await sleep(3000)
    const before = await reading()
    row('before', `pos=${before.pos?.map(f1)}`)
    await act(B)
    const tAct = now()
    const d0 = before.pos ? dist2(before.pos, [B.x + 0.5, 0, B.z + 0.5]) : null
    let reachedMs = null
    let nearest = Infinity
    while (now() - tAct < 12_000) {
      const r = await reading()
      const db = r.pos ? dist2(r.pos, [B.x + 0.5, 0, B.z + 0.5]) : null
      if (db != null) {
        nearest = Math.min(nearest, db)
        if (db <= 2 && reachedMs == null) reachedMs = now() - tAct
      }
      row(`pos=${r.pos?.map(f1)}`, `toBell=${f1(db)}`, `anger=${r.anger}`)
      await sleep(500)
    }
    row('summary', `wardenToBellAtAct=${f1(d0)}`, `reached=${reachedMs != null}`, `reachedMs=${reachedMs}`, `nearestToBell=${f1(nearest)}`)
    await rcon.run(`setblock ${B.x} ${B.y} ${B.z} minecraft:air`)
    await rcon.probe(`setblock ${B.x + 1} ${B.y} ${B.z} minecraft:air`)
  }
  await probeBell('control-10', 10, async () => {})
  await probeBell('state-powered', 10, (B) =>
    rcon.run(`setblock ${B.x} ${B.y} ${B.z} minecraft:bell[attachment=floor,facing=north,powered=true]`),
  )
  await probeBell('playsound-only', 10, (B) =>
    rcon.run(`playsound minecraft:block.bell.use block @a ${B.x} ${B.y} ${B.z} 1 1`),
  )
  for (const d of [10, 15, 16, 18, 20]) {
    await probeBell(`redstone-block-${d}`, d, (B) => rcon.run(`setblock ${B.x + 1} ${B.y} ${B.z} minecraft:redstone_block`))
  }
}

// T6: does a command summon play the emerge animation? The bot, 12 blocks
// north, reads the warden's synced pose (entity metadata index 6; 13 =
// EMERGING, 14 = DIGGING, 0 = STANDING on this version) off its own client.
async function t6(bot) {
  await clearBodies()
  await rcon.run(`tp ${BOT} ${W.x} ${W.y} ${W.z - 12}`)
  await sleep(1000)
  const variants = [
    ['bare', ''],
    ['empty-nbt', '{}'],
    ['awake', awakeNbt()],
    ['is-emerging', awakeNbt('"minecraft:is_emerging":{value:{},ttl:133L}')],
  ]
  for (const [label, nbt] of variants) {
    await clearBodies()
    begin(`T6-emerge-${label}`, `summon ${nbt || '<no nbt>'}; pose read by the bot every 250 ms`)
    await bot.lookAt(new Vec3(W.x + 0.5, W.y + 1, W.z + 0.5), true)
    await summonWarden(W, nbt)
    const tS = now()
    const poses = []
    let lastPose
    while (now() - tS < 12_000) {
      const w = wardenEntity(bot)
      const pose = w ? w.metadata?.[6] : 'absent'
      if (pose !== lastPose) {
        poses.push(`${now() - tS}ms:${pose}`)
        lastPose = pose
      }
      await sleep(250)
    }
    const mem = await wardenGet('Brain.memories')
    const alive = (await wardenGet('Health')) != null
    row('poses', poses.join(' '))
    row('summary', `aliveAt12s=${alive}`, `memories=${mem}`)
  }
}

// ---- T7–T9: the native entrance -------------------------------------------------
// `execute if entity` answers the rcon source "Test passed, count: N" or
// "Test failed"; anything else is reported as it came, never guessed.
async function countEntities(sel) {
  const reply = await rcon.probe(`execute if entity ${sel}`)
  const m = reply.match(/count: (\d+)/)
  if (m) return Number(m[1])
  if (/Test failed/.test(reply)) return 0
  return `?(${reply})`
}
async function countStone(cells) {
  let n = 0
  for (const [x, y, z] of cells) {
    const reply = await rcon.probe(`execute if block ${x} ${y} ${z} minecraft:stone`)
    if (/Test passed/.test(reply)) n += 1
  }
  return n
}
const WITHER_SEL = '@e[type=minecraft:wither,limit=1]'
async function witherGet(path) {
  return valueOf(await rcon.probe(`data get entity ${WITHER_SEL} ${path}`))
}
// The wither ignores the undead (`EntityTypeTags.WITHER_FRIENDS`), so its target
// here is a villager. Resistance V: a valid target it cannot kill.
async function summonTarget(at, tag, extra = '') {
  await rcon.probe(`kill @e[tag=${tag}]`)
  await rcon.run(
    `summon minecraft:villager ${at.x} ${at.y} ${at.z} {NoAI:1b,Silent:1b,PersistenceRequired:1b,Tags:["${tag}"]${extra}}`,
  )
  await sleep(300)
}
async function shieldBot() {
  await rcon.run(`effect give ${BOT} minecraft:resistance infinite 4 true`)
}
async function unshieldBot() {
  await rcon.probe(`effect clear ${BOT} minecraft:resistance`)
}
// A ring of stone two blocks out from the body, three high: what the blast that
// ends the charge-up does to blocks is read as how many of these survive.
function ringCells(c) {
  const cells = []
  for (const [dx, dz] of [[2, 0], [-2, 0], [0, 2], [0, -2], [2, 2], [-2, -2], [2, -2], [-2, 2]]) {
    for (let dy = 0; dy < 3; dy++) cells.push([c.x + dx, c.y + dy, c.z + dz])
  }
  return cells
}
async function placeRing(c) {
  for (const [x, y, z] of ringCells(c)) await rcon.run(`setblock ${x} ${y} ${z} minecraft:stone`)
}
async function clearRing(c) {
  for (const [x, y, z] of ringCells(c)) await rcon.probe(`setblock ${x} ${y} ${z} minecraft:air`)
}

// T7: the wither's spawn charge-up. A block-built wither spends 220 ticks
// charging (`Invul`), healing from a third of its health, and ends it with a
// blast; a command summon carries whatever `Invul` the NBT says. Read over rcon
// every 500 ms: `Invul`, `Health`, feet, skulls in the air (its attack), the
// target's health; the bot reads the vanilla boss bar's progress off its client
// (a second instrument for the countdown that shares nothing with the NBT read).
async function t7(bot) {
  await clearBodies()
  // Past the wither's 40-block `follow_range`, inside its 10-chunk tracking range.
  await rcon.run(`tp ${BOT} ${W.x} ${W.y} ${W.z - 48}`)
  bot.clearControlStates()
  await shieldBot()
  await sleep(800)
  const bars = []
  const onBar = (bar) => bars.push(`${now() - t0}ms:${(bar.health ?? bar.percent ?? '?').toString().slice(0, 5)}`)
  bot.on('bossBarUpdated', onBar)
  bot.on('bossBarCreated', onBar)
  const T = { x: W.x + 8, y: W.y, z: W.z }
  const arms = [
    ['bare', '', 'mob_griefing false', 9_000],
    ['invul-220', '{PersistenceRequired:1b,Invul:220,Health:100f}', 'mob_griefing false', 15_000],
    ['invul-220-griefing', '{PersistenceRequired:1b,Invul:220}', 'mob_griefing true', 15_000],
    ['invul-220-noai', '{PersistenceRequired:1b,Invul:220,NoAI:1b}', 'mob_griefing false', 4_000],
  ]
  for (const [label, nbt, griefing, watchMs] of arms) {
    await clearBodies()
    await rcon.run(`gamerule ${griefing}`)
    await clearRing(W)
    await placeRing(W)
    // Two targets the blast can reach, at 3 and 6 blocks, with health to spare so
    // the damage is read as a number rather than a death.
    await summonTarget({ x: W.x + 3, y: W.y, z: W.z + 3 }, 'dw_t3', ',attributes:[{id:"minecraft:max_health",base:1024}],Health:1024f')
    await summonTarget({ x: W.x + 6, y: W.y, z: W.z }, 'dw_t6', ',attributes:[{id:"minecraft:max_health",base:1024}],Health:1024f')
    // The hunting target, unkillable, at 8 blocks.
    await summonTarget(T, 'dw_wtarget', ',active_effects:[{id:"minecraft:resistance",amplifier:4b,duration:-1,show_particles:0b}]')
    const ring0 = await countStone(ringCells(W))
    bars.length = 0
    begin(`T7-wither-${label}`, `summon wither ${nbt || '<no nbt>'}; ${griefing}; ring of ${ring0} stone at 2 blocks; targets at 3, 6 (1024 hp) and 8 (resistance V) blocks`)
    await rcon.run(`summon minecraft:wither ${W.x} ${W.y} ${W.z}${nbt ? ' ' + nbt : ''}`)
    const tS = now()
    let p0 = null
    let maxMove = 0
    let hitAt = null
    let blastRead = false
    let damaged = false
    while (now() - tS < watchMs) {
      const invul = await witherGet('Invul')
      const health = await witherGet('Health')
      const pos = parsePos(await witherGet('Pos'))
      if (pos && !p0) p0 = pos
      if (pos && p0) maxMove = Math.max(maxMove, Math.hypot(pos[0] - p0[0], pos[1] - p0[1], pos[2] - p0[2]))
      const skulls = await countEntities('@e[type=minecraft:wither_skull]')
      row(`t=${((now() - tS) / 1000).toFixed(1)}s`, `invul=${invul}`, `health=${health}`, `moved=${maxMove.toFixed(2)}`, `skulls=${skulls}`)
      if (health == null) {
        row('gone', 'the wither is no longer readable')
        break
      }
      // One blow of 10 at about 3 s: hurt, or refused while charging.
      if (!damaged && now() - tS > 3000) {
        damaged = true
        const before = await witherGet('Health')
        const reply = await rcon.probe(`damage ${WITHER_SEL} 10 minecraft:generic`)
        await sleep(150)
        const after = await witherGet('Health')
        row('blow', `health ${before} -> ${after}`, `reply=${reply}`)
      }
      // The tick the charge ends: read the blast's reach at once.
      if (!blastRead && invul != null && /^0\b/.test(invul) && nbt.includes('Invul')) {
        blastRead = true
        hitAt = now() - tS
        const h3 = await entityGet('@e[tag=dw_t3,limit=1]', 'Health')
        const h6 = await entityGet('@e[tag=dw_t6,limit=1]', 'Health')
        const ring1 = await countStone(ringCells(W))
        row('blast', `at=${hitAt}ms`, `target3=${h3}`, `target6=${h6}`, `ring ${ring0} -> ${ring1}`)
      }
      await sleep(500)
    }
    const tgt = parsePos(await entityGet('@e[tag=dw_wtarget,limit=1]', 'Pos'))
    const tHealth = await entityGet('@e[tag=dw_wtarget,limit=1]', 'Health')
    row('end', `targetAt8=${tgt ? 'present' : 'gone'} health=${tHealth}`, `bossbar=${bars.join(' ') || '-'}`)
    await rcon.probe('kill @e[tag=dw_t3]')
    await rcon.probe('kill @e[tag=dw_t6]')
    await rcon.probe('kill @e[tag=dw_wtarget]')
  }
  bot.removeListener('bossBarUpdated', onBar)
  bot.removeListener('bossBarCreated', onBar)
  await clearRing(W)
  await rcon.run('gamerule mob_griefing true')
  await unshieldBot()
}

// T8: the emerge window. The warden is summoned awake with `is_emerging`
// (T6's shape); the bot reads its pose while the server is asked what the body
// hears, takes and remembers during the 133 ticks.
async function t8(bot) {
  const emergingNbt = () => awakeNbt('"minecraft:is_emerging":{value:{},ttl:133L}')
  const readPose = () => {
    const w = wardenEntity(bot)
    return w ? w.metadata?.[6] : 'absent'
  }
  // 8a: a player walking four blocks from the emerging body. Resistance V keeps
  // the bot alive past the emerge so the hunt that follows is read too.
  await clearBodies()
  await shieldBot()
  await rcon.run(`tp ${BOT} ${W.x} ${W.y} ${W.z - 4}`)
  await sleep(800)
  begin('T8-emerge-hearing', 'bot walks to and fro 4 blocks north of an emerging warden; anger read every second')
  await bot.lookAt(new Vec3(W.x + 0.5, W.y + 1, W.z + 0.5), true)
  await summonWarden(W, emergingNbt())
  const tS = now()
  let lastPose
  let flip = false
  const uuid = bot.player?.uuid ?? bot.entity?.uuid
  while (now() - tS < 12_000) {
    const pose = readPose()
    if (pose !== lastPose) {
      row('pose', `${now() - tS}ms:${pose}`)
      lastPose = pose
    }
    // To and fro: walking is the vibration the hunter hears (T4).
    bot.setControlState('forward', false)
    bot.setControlState('back', false)
    bot.setControlState(flip ? 'forward' : 'back', true)
    flip = !flip
    if (Math.round((now() - tS) / 250) % 4 === 0) {
      const anger = await wardenGet('anger.suspects')
      const pos = parsePos(await wardenGet('Pos'))
      const me = bot.entity?.position
      row(`t=${((now() - tS) / 1000).toFixed(1)}s`, `pose=${pose}`, `anger=${anger}`, `wardenToBot=${pos && me ? f1(Math.hypot(pos[0] - me.x, pos[2] - me.z)) : '-'}`)
    }
    await sleep(250)
  }
  bot.clearControlStates()
  row('summary', `botUuid=${uuid}`, `anger=${await wardenGet('anger.suspects')}`, `botHealth=${bot.health}`)

  // 8b: a blow during the emerge.
  await clearBodies()
  await rcon.run(`tp ${BOT} ${W.x} ${W.y} ${W.z - 12}`)
  await sleep(800)
  begin('T8-emerge-blow', 'damage 10 generic at 2 s into the emerge; health and pose read')
  await bot.lookAt(new Vec3(W.x + 0.5, W.y + 1, W.z + 0.5), true)
  await summonWarden(W, emergingNbt())
  await sleep(2000)
  const before = await wardenGet('Health')
  const poseBefore = readPose()
  const reply = await rcon.probe(`damage @e[type=minecraft:warden,limit=1] 10 minecraft:generic`)
  await sleep(300)
  row('blow', `health ${before} -> ${await wardenGet('Health')}`, `pose ${poseBefore} -> ${readPose()}`, `reply=${reply}`)
  await sleep(5500)
  row('after', `pose=${readPose()}`, `health=${await wardenGet('Health')}`, `alive=${(await wardenGet('Health')) != null}`)

  // 8c: a seeded anger across the emerge — the aggro lock `unleash_<id>` writes,
  // on a body that cannot act for 133 ticks. Does the hunt follow?
  await clearBodies()
  await park(bot)
  const V = { x: W.x + 10, y: W.y, z: W.z }
  const victim = await summonVictim(V)
  begin('T8-emerge-anger-seeded', `anger 150 at a body 10 blocks east, seeded on an emerging warden; feet read every second`)
  await summonWarden(W, awakeNbt('"minecraft:is_emerging":{value:{},ttl:133L}', angerAt(victim, 150)))
  const t0c = now()
  let charged = null
  while (now() - t0c < 16_000) {
    const r = await reading()
    const d = r.pos ? dist2(r.pos, [V.x, V.y, V.z]) : null
    if (d != null && d < 2.5 && charged == null) charged = now() - t0c
    row(`t=${((now() - t0c) / 1000).toFixed(1)}s`, `wardenToVictim=${f1(d)}`, `anger=${r.anger}`)
    await sleep(1000)
  }
  row('summary', `reachedVictimAt=${charged == null ? 'never' : charged + 'ms'}`)
  await rcon.probe('kill @e[tag=dw_victim]')
  await unshieldBot()
}

// T9: a body summoned and removed in one tick. The bot, 12 blocks away, records
// every warden its client is ever told about and where it stood. Three shapes,
// each a datapack function the probe writes into the container: (a) summon then
// `kill` in one function; (b) summon then the compiler's unseen removal — a `tp`
// to Y -128 in the same function; (c) the control: summon alone, killed by a
// later command, which the client must see for the instrument to count.
async function t9(bot) {
  const b64 = (s) => Buffer.from(s, 'utf8').toString('base64')
  const dp = '/data/world/datapacks/dwprobe'
  const summon = `summon minecraft:warden ${W.x} ${W.y} ${W.z} {NoAI:1b,PersistenceRequired:1b,Tags:["dw_blink"],Brain:{memories:{${KEEP_AWAKE}}}}`
  const files = {
    'pack.mcmeta': JSON.stringify({ pack: { description: 'probe', min_format: [94, 1], max_format: [94, 1] } }),
    'data/dwprobe/function/blink_kill.mcfunction': `${summon}\nkill @e[tag=dw_blink]\n`,
    'data/dwprobe/function/blink_unseen.mcfunction': `${summon}\nexecute as @e[tag=dw_blink] at @s run tp @s ~ -128 ~\n`,
    'data/dwprobe/function/blink_control.mcfunction': `${summon}\n`,
  }
  for (const [rel, body] of Object.entries(files)) {
    const dir = `${dp}/${rel}`.replace(/\/[^/]+$/, '')
    await execFileP('docker', ['exec', container, 'sh', '-c', `mkdir -p '${dir}' && echo '${b64(body)}' | base64 -d > '${dp}/${rel}'`])
  }
  console.log(`#RELOAD\t${await rcon.probe('reload')}`)
  await clearBodies()
  await rcon.run(`tp ${BOT} ${W.x} ${W.y} ${W.z - 12}`)
  await sleep(800)
  const seen = []
  const onSpawn = (e) => {
    if (e?.name === 'warden') seen.push(`${now() - t0}ms@[${e.position?.x?.toFixed(1)},${e.position?.y?.toFixed(1)},${e.position?.z?.toFixed(1)}]`)
  }
  bot.on('entitySpawn', onSpawn)
  for (const fn of ['blink_kill', 'blink_unseen', 'blink_control']) {
    await clearBodies()
    seen.length = 0
    begin(`T9-${fn}`, `function dwprobe:${fn}; the bot records every warden spawn its client receives for 2 s`)
    const reply = await rcon.probe(`function dwprobe:${fn}`)
    await sleep(2000)
    const server = await countEntities('@e[tag=dw_blink]')
    const clientNow = Object.values(bot.entities).filter((e) => e.name === 'warden').length
    row('result', `reply=${reply}`, `clientSpawnsSeen=${seen.length}`, `where=${seen.join(' ') || '-'}`, `serverBodies=${server}`, `clientBodiesNow=${clientNow}`)
    await rcon.probe('kill @e[tag=dw_blink]')
  }
  bot.removeListener('entitySpawn', onSpawn)
  await execFileP('docker', ['exec', container, 'rm', '-rf', dp])
  await rcon.probe('reload')
}

// ---- run --------------------------------------------------------------------
async function setup() {
  await rcon.run('gamerule spawn_mobs false')
  await rcon.run('gamerule random_tick_speed 0')
  await rcon.run('difficulty easy')
  await rcon.run('time set noon')
  await rcon.run(`forceload add ${W.x - 48} ${W.z - 48} ${W.x + 48} ${W.z + 48}`)
  await rcon.run(`fill ${W.x - 45} ${FLOOR_Y} ${W.z - 45} ${W.x + 45} ${FLOOR_Y} ${W.z + 45} minecraft:stone`)
}
async function teardown(bot) {
  try {
    await clearBodies()
    await rcon.probe('kill @e[tag=dw_victim]')
  } catch (e) {
    console.error(`teardown: ${e.message}`)
  }
  try {
    // The arena is wider than one fill may clear (32768 blocks), so it is
    // cleared in strips of 25 x 13 x 91 (an already-clear strip answers "No blocks were filled", which is not a failure here).
    for (let x = W.x - 45; x <= W.x + 45; x += 25) {
      await rcon.probe(`fill ${x} ${FLOOR_Y} ${W.z - 45} ${Math.min(x + 24, W.x + 45)} ${FLOOR_Y + 12} ${W.z + 45} minecraft:air`)
    }
    await rcon.probe('forceload remove all')
  } catch (e) {
    console.error(`teardown: ${e.message}`)
  }
  try { bot?.quit('probe done') } catch { /* gone */ }
}

let bot
try {
  console.log(`== warden maze probe: ${await rcon.probe('version')} ==`)
  await setup()
  bot = await joinBot()
  console.log(`#BOT\tjoined as ${bot.username} uuid=${bot.player?.uuid ?? bot.entity?.uuid}`)
  await rcon.run(`gamemode survival ${BOT}`)
  if (wants('T1')) await t1(bot)
  if (wants('T2')) await t2(bot)
  if (wants('T3')) await t3(bot)
  if (wants('T4')) await t4(bot)
  if (wants('T5')) await t5(bot)
  if (wants('T6')) await t6(bot)
  if (wants('T7')) await t7(bot)
  if (wants('T8')) await t8(bot)
  if (wants('T9')) await t9(bot)
  console.log('== probe complete ==')
} catch (e) {
  console.error(`probe failed: ${e.stack ?? e}`)
  process.exitCode = 1
} finally {
  await teardown(bot)
  setTimeout(() => process.exit(), 1500)
}
