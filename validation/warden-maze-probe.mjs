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
//   T6  whether a command summon can play the emerge-from-the-ground animation.
//
// One mineflayer bot is the "player" where a player is the question (T2, T4, T6:
// it reads the warden's synced pose off its own client). Everything else is read
// back over rcon through the repo's one rejection rule (`tools/lib/rcon.mjs`).
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
// reads. The written record is `docs/reference/warden-behaviour.md`.
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
  console.log('== probe complete ==')
} catch (e) {
  console.error(`probe failed: ${e.stack ?? e}`)
  process.exitCode = 1
} finally {
  await teardown(bot)
  setTimeout(() => process.exit(), 1500)
}
