// What the pinned 1.21.11 server does with the things a festival-day level
// rests on — measured on a THROWAWAY pinned server, never the shared stack (the
// pattern of `warden-maze-probe.mjs`). Each question is answered by running it:
//
//   T1  a target block shot with a bow from 27 blocks: does `minecraft:target_hit`
//       fire for the shooter, for a snowball, for an arrow nobody shot, and can a
//       mineflayer bot land the hit;
//   T2  a firework rocket's burst damage by star count, and one fired from a
//       crossbow the bot loads;
//   T3  a goat beside a player holding wheat;
//   T4  a saddled pig under a rider holding a carrot on a stick, and `/ride`;
//   T5  a boat: mounting, what moves it with a passenger, where a dismount puts
//       the body, `execute on vehicle`;
//   T6  a minecart on rails: the same questions, plus a powered rail;
//   T7  a box selector's count over bodies, and a body on the box's edge;
//   T8  `effect give` and `damage` aimed at a mob;
//   T9  `minecraft:using_item` for a spyglass.
//
// One mineflayer bot is the player wherever a player is the question. Everything
// else is read back over rcon through the repo's one rejection rule
// (`tools/lib/rcon.mjs`). The datapack the advancement trials need is written
// into the container by this script and removed with it.
//
//   docker run -d --name dw-<round>-festival --cpus=2 --memory=2g -e EULA=TRUE \
//     -e TYPE=VANILLA -e VERSION=1.21.11 -e DIFFICULTY=easy \
//     -e LEVEL_TYPE=minecraft:flat -e SPAWN_MONSTERS=false -e SPAWN_ANIMALS=false \
//     -e ONLINE_MODE=FALSE -e ENABLE_RCON=TRUE -e RCON_PASSWORD=<pw> \
//     ghcr.io/stellarfeline/delvewright-base@sha256:<versions.toml [images.base].digest>
//   node validation/festival-probe.mjs dw-<round>-festival [only=T1,T5]
//
// The bot joins the container's own address (no host port is published), so the
// script runs on the Docker host. mineflayer is the harness's pinned copy,
// resolved from `harness/` (run `npm ci` there first).
//
// Output: one TSV line per reading — trial, elapsed ms, and the fields the trial
// reads. The written record is `docs/reference/vehicles-animals-and-targets.md`.
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
  console.error('usage: node validation/festival-probe.mjs <container> [only=T1,T2,...]')
  process.exit(2)
}
const onlyArg = process.argv.slice(3).find((a) => a.startsWith('only='))
const only = onlyArg ? new Set(onlyArg.slice(5).split(',')) : null
const wants = (t) => !only || only.has(t)

const rcon = rconChannel(container)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const now = () => Date.now()

// ---- arena ------------------------------------------------------------------
// A stone pad at y=100 in a superflat world (its surface is at y=-60, so
// everything over the pad is open air and open sky). The bot's home is H.
const FLOOR_Y = 100
const H = { x: 200, y: 101, z: 200 }
const PAD = 60
const BOT = 'dw-fest-probe'
const OBJ = 'probe'

function valueOf(reply) {
  const m = reply.match(/entity data: (.*)$/s)
  return m ? m[1].trim() : null
}
async function entityGet(sel, path) {
  return valueOf(await rcon.probe(`data get entity ${sel} ${path}`))
}
function parsePos(s) {
  if (!s) return null
  const m = s.match(/\[([-\d.]+)d, ([-\d.]+)d, ([-\d.]+)d\]/)
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null
}
function parseNum(s) {
  if (!s) return null
  const m = s.match(/([-\d.]+)[fdbsL]?$/)
  return m ? Number(m[1]) : null
}
const dist2 = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2])
const f1 = (n) => (n == null ? '-' : Number(n).toFixed(1))
const f2 = (n) => (n == null ? '-' : Number(n).toFixed(2))

async function score(holder) {
  const r = await rcon.probe(`scoreboard players get ${holder} ${OBJ}`)
  const m = r.match(/ has (-?\d+) \[/)
  return m ? Number(m[1]) : 0
}
async function resetScore(holder) {
  await rcon.probe(`scoreboard players reset ${holder} ${OBJ}`)
}
/** A count over a selector, stored into a score and read back (`execute if entity` alone answers rcon nothing). */
async function countOf(selector) {
  await rcon.probe(`execute store result score #count ${OBJ} if entity ${selector}`)
  return score('#count')
}

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

// ---- the datapack the advancement trials read ---------------------------------
// Written into the container's world. `minecraft:target_hit` and
// `minecraft:using_item` are advancement criteria, so the only way to see them
// is an advancement whose reward counts, then re-arms itself.
const PACK = '/data/world/datapacks/dw_probe'
async function writeInContainer(path, content) {
  // The content travels as an argument: `execFile` has no stdin to feed `cat`.
  await execFileP('docker', ['exec', container, 'sh', '-c', 'mkdir -p "$(dirname "$1")" && printf %s "$2" > "$1"', 'sh', path, content])
}
async function installPack() {
  await writeInContainer(`${PACK}/pack.mcmeta`, JSON.stringify({ pack: { pack_format: 94, description: 'festival probe' } }))
  const adv = (trigger, conditions, fn) =>
    JSON.stringify({ criteria: { c: { trigger, conditions } }, rewards: { function: `dw_probe:${fn}` } })
  await writeInContainer(`${PACK}/data/dw_probe/advancement/target_any.json`, adv('minecraft:target_hit', {}, 'target_any'))
  await writeInContainer(`${PACK}/data/dw_probe/advancement/target_strong.json`, adv('minecraft:target_hit', { signal_strength: { min: 8 } }, 'target_strong'))
  await writeInContainer(`${PACK}/data/dw_probe/advancement/spyglass.json`, adv('minecraft:using_item', { item: { items: 'minecraft:spyglass' } }, 'spyglass'))
  for (const n of ['target_any', 'target_strong', 'spyglass']) {
    await writeInContainer(`${PACK}/data/dw_probe/function/${n}.mcfunction`, `scoreboard players add #${n} ${OBJ} 1\nadvancement revoke @s only dw_probe:${n}\n`)
  }
  const r = await rcon.probe('reload')
  row('reload', r)
  const list = await rcon.probe('datapack list')
  if (!list.includes('dw_probe')) {
    const en = await rcon.probe('datapack enable "file/dw_probe"')
    row('datapack enable', en)
  }
  const after = await rcon.probe('datapack list')
  if (!after.includes('dw_probe')) throw new Error(`datapack not enabled: ${after}`)
  row('datapack list', after)
}

async function setup() {
  await rcon.run('gamerule spawn_mobs false')
  await rcon.run('gamerule random_tick_speed 0')
  await rcon.run('gamerule advance_time false')
  await rcon.run('time set noon')
  await rcon.run(`forceload add ${H.x - PAD - 8} ${H.z - PAD - 8} ${H.x + PAD + 8} ${H.z + PAD + 8}`)
  // 121 × 121 of stone, in strips a `fill` admits (32768 cells).
  for (let x = H.x - PAD; x <= H.x + PAD; x += 20) {
    await rcon.run(`fill ${x} ${FLOOR_Y} ${H.z - PAD} ${Math.min(x + 19, H.x + PAD)} ${FLOOR_Y} ${H.z + PAD} minecraft:stone`)
  }
  await rcon.probe(`scoreboard objectives add ${OBJ} dummy`)
  await installPack()
}

// ---- the bot ----------------------------------------------------------------
async function containerIp() {
  const { stdout } = await execFileP('docker', ['inspect', '-f', '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}', container])
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
  bot.on('mount', () => console.log(`#BOT\tmount event`))
  bot.on('dismount', () => console.log(`#BOT\tdismount event`))
  return bot
}
async function home(bot, at = H) {
  await rcon.run(`tp ${BOT} ${at.x} ${at.y} ${at.z}`)
  bot.clearControlStates()
  await sleep(800)
}
async function heal() {
  await rcon.probe(`effect give ${BOT} minecraft:instant_health 1 10 true`)
  await rcon.probe(`effect give ${BOT} minecraft:saturation 1 10 true`)
}
async function hold(bot, itemName) {
  await rcon.run(`clear ${BOT}`)
  await sleep(300)
  if (!itemName) return
  await rcon.run(`give ${BOT} minecraft:${itemName}`)
  await sleep(600)
  const item = bot.inventory.items().find((i) => i.name === itemName)
  if (!item) throw new Error(`bot was given ${itemName} and holds none`)
  await bot.equip(item, 'hand')
  await sleep(300)
}
function entityNamed(bot, name) {
  return Object.values(bot.entities).find((e) => e.name === name)
}
async function botPos() {
  return parsePos(await entityGet(BOT, 'Pos'))
}
/** The client-side vehicle the bot believes it is in, and the server's word. */
async function rideState(bot) {
  const server = await entityGet(BOT, 'RootVehicle.Entity.id')
  return `clientVehicle=${bot.vehicle?.name ?? 'none'} serverRootVehicle=${server ?? 'none'}`
}
async function clearBodies() {
  for (const t of ['arrow', 'snowball', 'firework_rocket', 'item', 'experience_orb', 'goat', 'pig', 'oak_boat', 'minecart', 'chicken']) {
    await rcon.probe(`kill @e[type=minecraft:${t}]`)
  }
  await sleep(600)
}

// ---- T1: the target block ----------------------------------------------------
async function t1(bot) {
  await home(bot)
  await clearBodies()
  const T = { x: H.x + 27, y: H.y + 1, z: H.z }
  begin('T1-target-bow', `a target block ${T.x - H.x} blocks east, its centre one block over the bot's feet; the bot draws a bow for 1.2 s and releases, aimed at the centre plus an elevation`)
  await rcon.run(`setblock ${T.x} ${T.y} ${T.z} minecraft:target`)
  // A backstop so a missed arrow is seen to miss rather than fly off.
  await rcon.run(`fill ${T.x + 2} ${FLOOR_Y + 1} ${T.z - 4} ${T.x + 2} ${FLOOR_Y + 8} ${T.z + 4} minecraft:stone`)
  await rcon.run(`gamemode survival ${BOT}`)
  await heal()
  await hold(bot, 'bow')
  await rcon.run(`give ${BOT} minecraft:arrow 64`)
  await sleep(500)
  await resetScore('#target_any')
  await resetScore('#target_strong')
  const centre = new Vec3(T.x + 0.5, T.y + 0.5, T.z + 0.5)
  for (const elev of [0, 1, 1.5, 2, 2.5, 3, 4]) {
    for (let k = 0; k < 2; k++) {
      await rcon.probe('kill @e[type=minecraft:arrow]')
      const before = await score('#target_any')
      const beforeStrong = await score('#target_strong')
      await bot.lookAt(centre.offset(0, elev, 0), true)
      await sleep(300)
      bot.activateItem()
      await sleep(1200)
      bot.deactivateItem()
      await sleep(1500)
      const arrows = await countOf('@e[type=minecraft:arrow]')
      const inTarget = await countOf(`@e[type=minecraft:arrow,x=${T.x - 1},y=${T.y - 1},z=${T.z - 1},dx=2,dy=2,dz=2]`)
      const after = await score('#target_any')
      const afterStrong = await score('#target_strong')
      row(`elev=${elev}`, `shot=${k}`, `arrowsInWorld=${arrows}`, `arrowsAtTarget=${inTarget}`, `target_hit=${after - before}`, `target_hit(strength>=8)=${afterStrong - beforeStrong}`, `pitchDeg=${(bot.entity.pitch * 180 / Math.PI).toFixed(1)}`)
    }
  }

  begin('T1-target-snowball', 'the same target; the bot throws a snowball at it from 6 blocks')
  await home(bot, { x: T.x - 6, y: H.y, z: H.z })
  await hold(bot, 'snowball')
  for (let k = 0; k < 3; k++) {
    await rcon.probe('kill @e[type=minecraft:snowball]')
    const before = await score('#target_any')
    await bot.lookAt(centre.offset(0, 0.3, 0), true)
    await sleep(300)
    bot.activateItem()
    await sleep(100)
    bot.deactivateItem()
    await sleep(1200)
    const after = await score('#target_any')
    row(`throw=${k}`, `target_hit=${after - before}`)
  }

  begin('T1-target-sourceless', 'an arrow summoned 6 blocks west of the target with Motion toward it; no player shot it')
  await home(bot)
  for (let k = 0; k < 3; k++) {
    await rcon.probe('kill @e[type=minecraft:arrow]')
    const before = await score('#target_any')
    await rcon.run(`summon minecraft:arrow ${T.x - 6} ${T.y + 0.5} ${T.z + 0.5} {Motion:[2.0d,0.1d,0.0d]}`)
    await sleep(1200)
    const inTarget = await countOf(`@e[type=minecraft:arrow,x=${T.x - 1},y=${T.y - 1},z=${T.z - 1},dx=2,dy=2,dz=2]`)
    const after = await score('#target_any')
    row(`summon=${k}`, `arrowsAtTarget=${inTarget}`, `target_hit=${after - before}`)
  }
  await rcon.probe(`setblock ${T.x} ${T.y} ${T.z} minecraft:air`)
  await rcon.probe(`fill ${T.x + 2} ${FLOOR_Y + 1} ${T.z - 4} ${T.x + 2} ${FLOOR_Y + 8} ${T.z + 4} minecraft:air`)
}

// ---- T2: rockets --------------------------------------------------------------
const rocketItem = (stars) => {
  const explosions = Array.from({ length: stars }, () => '{shape:"small_ball",colors:[I;16711680]}').join(',')
  return `{id:"minecraft:firework_rocket",count:1,components:{"minecraft:fireworks":{flight_duration:1,explosions:[${explosions}]}}}`
}
async function summonDummy(at, tag) {
  await rcon.probe(`kill @e[tag=${tag}]`)
  await rcon.run(`summon minecraft:pig ${at.x} ${at.y} ${at.z} {NoAI:1b,Silent:1b,PersistenceRequired:1b,Tags:["${tag}"],attributes:[{id:"minecraft:max_health",base:200d}],Health:200f}`)
  await sleep(400)
  return parseNum(await entityGet(`@e[tag=${tag},limit=1]`, 'Health'))
}
async function t2(bot) {
  await home(bot)
  await clearBodies()
  begin('T2-rocket-stars', 'a rocket summoned inside a NoAI pig (200 health) with LifeTime 1, by star count 0..7; the pig\'s health read 1.5 s later')
  const D = { x: H.x + 20.5, y: H.y, z: H.z + 0.5 }
  for (const stars of [0, 1, 2, 3, 4, 5, 6, 7]) {
    const before = await summonDummy(D, 'dw_rk')
    await rcon.run(`summon minecraft:firework_rocket ${D.x} ${D.y + 0.4} ${D.z} {LifeTime:1,ShotAtAngle:1b,FireworksItem:${rocketItem(stars)}}`)
    await sleep(1500)
    const after = parseNum(await entityGet('@e[tag=dw_rk,limit=1]', 'Health'))
    row(`stars=${stars}`, `healthBefore=${before}`, `healthAfter=${after}`, `damage=${before != null && after != null ? f1(before - after) : 'dead?'}`)
  }

  begin('T2-rocket-crossbow', 'the bot loads a crossbow with a 3-star rocket from its off hand (1.5 s charge, released), then fires at the pig 6 blocks east; the bot stands 6 blocks off')
  await rcon.run(`gamemode survival ${BOT}`)
  await heal()
  await hold(bot, 'crossbow')
  await rcon.run(`item replace entity ${BOT} weapon.offhand with minecraft:firework_rocket[fireworks={flight_duration:1,explosions:[{shape:"small_ball",colors:[I;16711680]},{shape:"small_ball",colors:[I;16711680]},{shape:"small_ball",colors:[I;16711680]}]}] 8`)
  await sleep(500)
  const near = { x: H.x + 6.5, y: H.y, z: H.z + 0.5 }
  const before = await summonDummy(near, 'dw_rk')
  await bot.lookAt(new Vec3(near.x, near.y + 0.6, near.z), true)
  await sleep(300)
  bot.activateItem()
  await sleep(1500)
  bot.deactivateItem()
  await sleep(500)
  const loaded = await rcon.probe(`execute if items entity ${BOT} weapon.mainhand minecraft:crossbow[charged_projectiles~{id:"minecraft:firework_rocket"}]`)
  row('loaded', loaded)
  const botBefore = bot.health
  await bot.lookAt(new Vec3(near.x, near.y + 0.6, near.z), true)
  await sleep(200)
  bot.activateItem()
  await sleep(150)
  bot.deactivateItem()
  await sleep(2000)
  const after = parseNum(await entityGet('@e[tag=dw_rk,limit=1]', 'Health'))
  const rockets = await countOf('@e[type=minecraft:firework_rocket]')
  row(`pigHealthBefore=${before}`, `pigHealthAfter=${after}`, `damage=${before != null && after != null ? f1(before - after) : '-'}`, `botHealthBefore=${botBefore}`, `botHealthAfter=${bot.health}`, `rocketsStillFlying=${rockets}`)
  await rcon.probe('kill @e[tag=dw_rk]')
}

// ---- T3: a goat and wheat ------------------------------------------------------
async function t3(bot) {
  await home(bot)
  await clearBodies()
  await rcon.run(`gamemode survival ${BOT}`)
  const trials = [['T3-goat-wheat', 'wheat'], ['T3-goat-control', 'stone'], ['T3-goat-wheat-2', 'wheat'], ['T3-goat-control-2', 'stone'], ['T3-goat-wheat-3', 'wheat'], ['T3-goat-control-3', 'stone']]
  for (const [name, item] of trials) {
    await clearBodies()
    await home(bot)
    await hold(bot, item)
    begin(name, `the bot holds ${item}; a goat summoned 9 blocks south; distance read every 0.5 s for 12 s; then the bot is moved 8 blocks west and the goat read for 10 s more`)
    const G = { x: H.x + 0.5, y: H.y, z: H.z + 9.5 }
    await rcon.run(`summon minecraft:goat ${G.x} ${G.y} ${G.z} {PersistenceRequired:1b,Tags:["dw_goat"]}`)
    await bot.lookAt(new Vec3(G.x, G.y + 1, G.z), true)
    const read = async (ms, label) => {
      const tStart = now()
      let nearest = Infinity
      let last = null
      while (now() - tStart < ms) {
        const p = parsePos(await entityGet('@e[tag=dw_goat,limit=1]', 'Pos'))
        const b = await botPos()
        last = p && b ? dist2(p, b) : null
        if (last != null) nearest = Math.min(nearest, last)
        row(label, `goatPos=${p?.map(f1)}`, `toBot=${f1(last)}`)
        await sleep(500)
      }
      return { nearest, last }
    }
    const a = await read(12_000, 'phase1')
    await rcon.run(`tp ${BOT} ${H.x - 8} ${H.y} ${H.z}`)
    await bot.lookAt(new Vec3(G.x, G.y + 1, G.z), true)
    const b = await read(10_000, 'phase2-botMoved8West')
    row('summary', `phase1Nearest=${f1(a.nearest)}`, `phase1Final=${f1(a.last)}`, `phase2Nearest=${f1(b.nearest)}`, `phase2Final=${f1(b.last)}`, `followed=${b.last != null && b.last <= 3.5}`)
  }
}

// ---- T4: a pig under a rider ---------------------------------------------------
async function tryDismount(bot) {
  // mineflayer's `dismount` sends `jump` on the 1.21 input packet; the game's
  // dismount key is sneak. Both are tried, each read on the server.
  bot.dismount()
  await sleep(800)
  const afterJump = await rideState(bot)
  if (!bot.vehicle) return `jumpInput: dismounted (${afterJump})`
  bot._client.write('player_input', { inputs: { shift: true } })
  await sleep(400)
  bot._client.write('player_input', { inputs: { shift: false } })
  await sleep(600)
  const afterShift = await rideState(bot)
  return `jumpInput: still mounted (${afterJump}); shiftInput: ${bot.vehicle ? 'still mounted' : 'dismounted'} (${afterShift})`
}
async function mountByClick(bot, name) {
  const e = entityNamed(bot, name)
  if (!e) throw new Error(`the bot's client sees no ${name}`)
  await bot.lookAt(e.position.offset(0, 0.5, 0), true)
  await sleep(200)
  bot.mount(e)
  await sleep(1000)
  return rideState(bot)
}
let riseHook = null
async function t4(bot) {
  await home(bot)
  await clearBodies()
  await rcon.run(`gamemode survival ${BOT}`)
  begin('T4-pig-carrot', 'a saddled pig 2 blocks east; the bot right-clicks it, holds a carrot on a stick, looks east; the pig\'s position read every 0.5 s for 6 s')
  const P = { x: H.x + 2.5, y: H.y, z: H.z + 0.5 }
  await rcon.run(`summon minecraft:pig ${P.x} ${P.y} ${P.z} {PersistenceRequired:1b,Tags:["dw_pig"],equipment:{saddle:{id:"minecraft:saddle",count:1}}}`)
  await sleep(600)
  await hold(bot, 'carrot_on_a_stick')
  row('mount', await mountByClick(bot, 'pig'))
  const start = parsePos(await entityGet('@e[tag=dw_pig,limit=1]', 'Pos'))
  row(`botPosOnServer=${(await botPos())?.map(f2)}`, `pigPos=${start?.map(f2)}`)
  await bot.lookAt(new Vec3(H.x + 60, H.y + 1.5, H.z + 0.5), true)
  const tStart = now()
  while (now() - tStart < 6000) {
    const p = parsePos(await entityGet('@e[tag=dw_pig,limit=1]', 'Pos'))
    row(`pigPos=${p?.map(f1)}`, `moved=${start && p ? f1(dist2(start, p)) : '-'}`, `dx=${start && p ? f1(p[0] - start[0]) : '-'}`)
    await sleep(500)
  }
  begin('T4-pig-no-carrot', 'the same ride, the hand emptied; 4 s')
  await rcon.run(`clear ${BOT}`)
  await sleep(500)
  const s2 = parsePos(await entityGet('@e[tag=dw_pig,limit=1]', 'Pos'))
  const t2s = now()
  while (now() - t2s < 4000) {
    const p = parsePos(await entityGet('@e[tag=dw_pig,limit=1]', 'Pos'))
    row(`pigPos=${p?.map(f1)}`, `moved=${s2 && p ? f1(dist2(s2, p)) : '-'}`)
    await sleep(500)
  }
  begin('T4-pig-client-move', 'the bot reports the pig\'s position 0.2 blocks east per tick for 2 s, the way a client steers; the server\'s reading of the pig')
  await hold(bot, 'carrot_on_a_stick')
  await clientDrive(bot, 'dw_pig', 'flat', 1, 0, 2000)
  // Obstacles are built ahead of where the pig now stands; the pig is never
  // teleported between trials, because the harness's client cannot parse this
  // version's entity teleport and would report from a stale position.
  const obstacle = async (name, note, build, ms) => {
    begin(name, note)
    const p0 = parsePos(await entityGet('@e[tag=dw_pig,limit=1]', 'Pos'))
    const sx = Math.floor(p0[0]) + 3
    const sz = Math.floor(p0[2])
    for (const line of build(sx, sz).split('\n')) await rcon.run(line)
    const x0 = bot.vehicle?.position?.x
    const reads = []
    const watcher = (async () => {
      const t = now()
      while (now() - t < ms + 600) {
        const p = parsePos(await entityGet('@e[tag=dw_pig,limit=1]', 'Pos'))
        if (p) reads.push(`${f2(p[0])}/${f2(p[1])}`)
        await sleep(200)
      }
    })()
    const p1 = await clientDrive(bot, 'dw_pig', `obstacleAtX=${sx} clientStartX=${f2(x0)}`, 1, 0, ms, 0.2, riseHook ? riseHook(sx) : null)
    riseHook = null
    await watcher
    row(`serverXYDuring=${reads.join(' ')}`)
    row(`pastObstacle=${p1 ? (p1[0] > sx + 1 ? 'yes' : 'no') : '-'}`, `serverX=${f2(p1?.[0])}`, `serverY=${f2(p1?.[1])}`, `clientX=${f2(bot.vehicle?.position?.x)}`)
    await rcon.probe(`fill ${sx} ${FLOOR_Y + 1} ${sz - 2} ${sx + 1} ${FLOOR_Y + 2} ${sz + 2} minecraft:air`)
  }
  await obstacle('T4-pig-client-full-step', 'a one-block step 3 blocks ahead; reports at the pig\'s own height for 2 s', (x, z) => `fill ${x} ${FLOOR_Y + 1} ${z - 2} ${x} ${FLOOR_Y + 1} ${z + 2} minecraft:stone`, 2000)
  await obstacle('T4-pig-client-wall', 'a two-block wall 3 blocks ahead; the same reports for 2 s', (x, z) => `fill ${x} ${FLOOR_Y + 1} ${z - 2} ${x} ${FLOOR_Y + 2} ${z + 2} minecraft:stone`, 2000)
  await obstacle('T4-pig-client-slab-step', 'a bottom slab 3 blocks ahead, then a full block; the same reports for 2 s', (x, z) => `fill ${x} ${FLOOR_Y + 1} ${z - 2} ${x} ${FLOOR_Y + 1} ${z + 2} minecraft:stone_slab\nfill ${x + 1} ${FLOOR_Y + 1} ${z - 2} ${x + 1} ${FLOOR_Y + 1} ${z + 2} minecraft:stone`, 2000)
  riseHook = (sx) => (x, y) => (x >= sx + 1.5 ? y + 1.0 : x >= sx + 0.5 ? y + 0.5 : y)
  await obstacle('T4-pig-client-slab-rise', 'a bottom slab 3 blocks ahead, then a full block; reports that rise 0.5 at the slab and 1.0 at the block', (x, z) => `fill ${x} ${FLOOR_Y + 1} ${z - 2} ${x} ${FLOOR_Y + 1} ${z + 2} minecraft:stone_slab\nfill ${x + 1} ${FLOOR_Y + 1} ${z - 2} ${x + 1} ${FLOOR_Y + 1} ${z + 2} minecraft:stone`, 2000)
  for (const [name, note, build] of [
    ['T4-pig-full-step', 'a one-block step across the pig\'s way east, 5 blocks ahead; 6 s of driving', (x, z) => `fill ${x} ${FLOOR_Y + 1} ${z - 2} ${x} ${FLOOR_Y + 1} ${z + 2} minecraft:stone`],
    ['T4-pig-slab-step', 'a bottom slab then a full block, as two half steps; 6 s of driving', (x, z) => `fill ${x} ${FLOOR_Y + 1} ${z - 2} ${x} ${FLOOR_Y + 1} ${z + 2} minecraft:stone_slab\nfill ${x + 1} ${FLOOR_Y + 1} ${z - 2} ${x + 1} ${FLOOR_Y + 1} ${z + 2} minecraft:stone`],
  ]) {
    begin(name, note)
    const p0 = parsePos(await entityGet('@e[tag=dw_pig,limit=1]', 'Pos'))
    const sx = Math.floor(p0[0]) + 5
    const sz = Math.floor(p0[2])
    for (const line of build(sx, sz).split('\n')) await rcon.run(line)
    await hold(bot, 'carrot_on_a_stick')
    await bot.lookAt(new Vec3(H.x + 80, H.y + 1.5, p0[2]), true)
    const ts = now()
    while (now() - ts < 6000) {
      const p = parsePos(await entityGet('@e[tag=dw_pig,limit=1]', 'Pos'))
      row(`pigPos=${p?.map(f2)}`, `stepAtX=${sx}`, `pastStep=${p ? (p[0] > sx + 1 ? 'yes' : 'no') : '-'}`)
      await sleep(500)
    }
    await rcon.probe(`fill ${sx} ${FLOOR_Y + 1} ${sz - 2} ${sx + 1} ${FLOOR_Y + 1} ${sz + 2} minecraft:air`)
  }
  begin('T4-pig-dismount', 'the bot dismounts: mineflayer\'s dismount (jump), then a sneak input')
  row(await tryDismount(bot))
  row(`botPosAfter=${(await botPos())?.map(f2)}`)
  begin('T4-pig-ride-command', '`ride <bot> mount <pig>` then `ride <bot> dismount`, each read on the server and the client')
  const m = await rcon.probe(`ride ${BOT} mount @e[tag=dw_pig,limit=1]`)
  await sleep(800)
  row(`mountReply=${m}`, await rideState(bot))
  const dm = await rcon.probe(`ride ${BOT} dismount`)
  await sleep(800)
  row(`dismountReply=${dm}`, await rideState(bot))
}

// ---- T5: a boat -----------------------------------------------------------------
async function readVehicle(bot, tag) {
  const v = parsePos(await entityGet(`@e[tag=${tag},limit=1]`, 'Pos'))
  const b = await botPos()
  return { v, b }
}
async function driveFor(bot, ms, tag, label) {
  const t = now()
  const { v: v0 } = await readVehicle(bot, tag)
  while (now() - t < ms) {
    bot.moveVehicle(0, 1)
    await sleep(50)
    if ((now() - t) % 500 < 60) {
      const { v, b } = await readVehicle(bot, tag)
      row(label, `vehiclePos=${v?.map(f2)}`, `botPos=${b?.map(f2)}`, `moved=${v0 && v ? f2(dist2(v0, v)) : '-'}`)
    }
  }
  const { v } = await readVehicle(bot, tag)
  row(label, 'summary', `moved=${v0 && v ? f2(dist2(v0, v)) : '-'}`)
}

/** The way a vanilla client moves a ridden body: it reports the vehicle's position. */
async function clientDrive(bot, tag, label, dx, dz, ms, perTick = 0.2, yAt = null) {
  const { v: s0, b: b0 } = await readVehicle(bot, tag)
  let x = bot.vehicle?.position?.x ?? s0[0]
  let z = bot.vehicle?.position?.z ?? s0[2]
  const y = bot.vehicle?.position?.y ?? s0[1]
  const yaw = bot.vehicle?.yaw ?? 0
  const t = now()
  let ticks = 0
  while (now() - t < ms) {
    x += dx * perTick
    z += dz * perTick
    ticks += 1
    bot._client.write('vehicle_move', { x, y: yAt ? yAt(x, y) : y, z, yaw, pitch: 0, onGround: true })
    await sleep(50)
  }
  await sleep(700)
  const { v: s1, b: b1 } = await readVehicle(bot, tag)
  row(label, `reported=${ticks} moves of ${perTick} (${f2(ticks * perTick)} blocks, y held at ${f2(y)})`, `vehiclePos=${s1?.map(f2)}`, `botPos=${b1?.map(f2)}`, `serverMoved=${s0 && s1 ? f2(dist2(s0, s1)) : '-'}`, `serverY=${f2(s1?.[1])}`, `clientVehiclePos=${bot.vehicle?.position?.toArray().map(f2)}`)
  return s1
}

async function t5(bot) {
  await home(bot)
  await clearBodies()
  await rcon.run(`gamemode survival ${BOT}`)
  begin('T5-boat', 'a basin 41 × 31 of water flush with the pad, one deep over stone; an oak boat 2 blocks south of the bot on the rim')
  const B = { x1: H.x - 20, z1: H.z + 25, x2: H.x + 20, z2: H.z + 55 }
  await rcon.run(`fill ${B.x1} ${FLOOR_Y} ${B.z1} ${B.x2} ${FLOOR_Y} ${B.z2} minecraft:water`)
  await rcon.run(`fill ${B.x1 - 1} ${FLOOR_Y - 1} ${B.z1 - 1} ${B.x2 + 1} ${FLOOR_Y - 1} ${B.z2 + 1} minecraft:stone`)
  await home(bot, { x: H.x + 0.5, y: H.y, z: B.z1 - 0.5 })
  await rcon.run(`summon minecraft:oak_boat ${H.x + 0.5} ${FLOOR_Y + 1} ${B.z1 + 1.5} {Tags:["dw_boat"]}`)
  await sleep(1000)
  const settled = parsePos(await entityGet('@e[tag=dw_boat,limit=1]', 'Pos'))
  row(`boatSettledPos=${settled?.map(f2)}`, 'the boat floats where the water puts it')
  await hold(bot, null)
  row('mount', await mountByClick(bot, 'oak_boat'))
  const { v, b } = await readVehicle(bot, 'dw_boat')
  row(`boatPos=${v?.map(f2)}`, `botPosOnServer=${b?.map(f2)}`, `botPosOnClient=${bot.entity.position.toArray().map(f2)}`)
  await rcon.probe(`execute store result score #onv ${OBJ} as ${BOT} on vehicle if entity @s[type=minecraft:oak_boat]`)
  row(`executeOnVehicleBoat=${await score('#onv')}`, `(1 = the rider's vehicle is an oak boat)`)
  const inBox = await countOf(`@a[name=${BOT},x=${B.x1},y=${FLOOR_Y - 1},z=${B.z1},dx=${B.x2 - B.x1},dy=4,dz=${B.z2 - B.z1}]`)
  row(`riderCountedInBasinBox=${inBox}`)
  await bot.lookAt(new Vec3(H.x + 0.5, FLOOR_Y + 1, B.z2), true)
  await driveFor(bot, 4000, 'dw_boat', 'forwardInput')
  // The way a vanilla client drives a boat: it computes the boat's physics and
  // reports the boat's position. Here the position is reported along a straight
  // line south at a stated speed, and the server's own reading of the boat says
  // whether it took the report.
  for (const [perTick, ms] of [[0.1, 1000], [0.25, 1000], [0.5, 1000], [1.0, 500]]) {
    const { v: s0 } = await readVehicle(bot, 'dw_boat')
    const t = now()
    let z = bot.vehicle?.position?.z ?? s0[2]
    const x = s0[0]
    let ticks = 0
    row(`speed ${(perTick * 20).toFixed(0)}b/s`, `clientBoatZ=${f2(bot.vehicle?.position?.z)}`, `serverBoatZ=${f2(s0?.[2])}`)
    while (now() - t < ms) {
      z += perTick
      ticks += 1
      bot._client.write('vehicle_move', { x, y: s0[1], z, yaw: 0, pitch: 0, onGround: true })
      await sleep(50)
    }
    await sleep(600)
    const { v: s1, b: b1 } = await readVehicle(bot, 'dw_boat')
    row(`clientVehicleMove(${perTick}/tick=${(perTick * 20).toFixed(0)}b/s)`, `reported=${ticks} moves, ${f2(ticks * perTick)} blocks`, `boatPos=${s1?.map(f2)}`, `botPos=${b1?.map(f2)}`, `serverMoved=${s0 && s1 ? f2(dist2(s0, s1)) : '-'}`, `clientBoatZ=${f2(bot.vehicle?.position?.z)}`)
  }
  await rcon.run(`execute as @e[tag=dw_boat,limit=1] run data merge entity @s {Motion:[0.0d,0.0d,0.6d]}`)
  await sleep(1500)
  const { v: v2, b: b2 } = await readVehicle(bot, 'dw_boat')
  row('motionMerge', `boatPos=${v2?.map(f2)}`, `botPos=${b2?.map(f2)}`, `moved=${v && v2 ? f2(dist2(v, v2)) : '-'}`)
  await rcon.run(`tp @e[tag=dw_boat,limit=1] ${H.x + 0.5} ${FLOOR_Y + 1} ${B.z1 + 12.5}`)
  await sleep(1000)
  const { v: v3, b: b3 } = await readVehicle(bot, 'dw_boat')
  row('tpVehicle', `boatPos=${v3?.map(f2)}`, `botPosOnServer=${b3?.map(f2)}`, `botPosOnClient=${bot.entity.position.toArray().map(f2)}`, await rideState(bot))
  // A chained teleport, the way a walk driver moves a puppet: ten steps of one block.
  for (let i = 1; i <= 10; i++) {
    await rcon.run(`tp @e[tag=dw_boat,limit=1] ${H.x + 0.5 + i} ${FLOOR_Y + 1} ${B.z1 + 12.5}`)
    await sleep(100)
  }
  await sleep(800)
  const { v: v4, b: b4 } = await readVehicle(bot, 'dw_boat')
  row('tpChain', `boatPos=${v4?.map(f2)}`, `botPosOnServer=${b4?.map(f2)}`, await rideState(bot))
  begin('T5-boat-dismount', 'the bot dismounts in open water: mineflayer\'s dismount (jump), then a sneak input; where the body is read')
  row(await tryDismount(bot))
  await sleep(1000)
  const after = await botPos()
  row(`botPosAfter=${after?.map(f2)}`, `boatPos=${parsePos(await entityGet('@e[tag=dw_boat,limit=1]', 'Pos'))?.map(f2)}`, `inWater=${after ? (after[1] < FLOOR_Y + 1 ? 'yes' : 'no') : '-'}`)
  begin('T5-boat-ride-command', '`ride` mounts and dismounts the bot into the boat')
  const m = await rcon.probe(`ride ${BOT} mount @e[tag=dw_boat,limit=1]`)
  await sleep(800)
  row(`mountReply=${m}`, await rideState(bot), `botPos=${(await botPos())?.map(f2)}`)
  const dm = await rcon.probe(`ride ${BOT} dismount`)
  await sleep(800)
  row(`dismountReply=${dm}`, await rideState(bot), `botPos=${(await botPos())?.map(f2)}`)
  await rcon.probe('kill @e[tag=dw_boat]')
  await rcon.probe(`fill ${B.x1} ${FLOOR_Y} ${B.z1} ${B.x2} ${FLOOR_Y} ${B.z2} minecraft:stone`)
}

// ---- T6: a minecart --------------------------------------------------------------
async function t6(bot) {
  await home(bot)
  await clearBodies()
  await rcon.run(`gamemode survival ${BOT}`)
  begin('T6-minecart', 'a straight rail 60 long on the pad, three powered rails on redstone blocks 6..8 in; a minecart at its west end, the bot 2 blocks south')
  const R = { x1: H.x - 55, x2: H.x + 5, z: H.z - 20 }
  await rcon.run(`fill ${R.x1} ${FLOOR_Y + 1} ${R.z} ${R.x2} ${FLOOR_Y + 1} ${R.z} minecraft:rail`)
  await rcon.run(`fill ${R.x1 + 6} ${FLOOR_Y} ${R.z} ${R.x1 + 8} ${FLOOR_Y} ${R.z} minecraft:redstone_block`)
  await rcon.run(`fill ${R.x1 + 6} ${FLOOR_Y + 1} ${R.z} ${R.x1 + 8} ${FLOOR_Y + 1} ${R.z} minecraft:powered_rail`)
  await rcon.run(`summon minecraft:minecart ${R.x1 + 2.5} ${FLOOR_Y + 1.1} ${R.z + 0.5} {Tags:["dw_cart"]}`)
  await home(bot, { x: R.x1 + 2.5, y: H.y, z: R.z + 2.5 })
  await hold(bot, null)
  row('mount', await mountByClick(bot, 'minecart'))
  const { v, b } = await readVehicle(bot, 'dw_cart')
  row(`cartPos=${v?.map(f2)}`, `botPosOnServer=${b?.map(f2)}`)
  await rcon.probe(`execute store result score #onv ${OBJ} as ${BOT} on vehicle if entity @s[type=minecraft:minecart]`)
  row(`executeOnVehicleMinecart=${await score('#onv')}`)
  await bot.lookAt(new Vec3(R.x2, FLOOR_Y + 1.5, R.z + 0.5), true)
  await driveFor(bot, 3000, 'dw_cart', 'forwardInput')
  await clientDrive(bot, 'dw_cart', 'clientVehicleMoveAlongRail', 1, 0, 2000)
  await rcon.run(`tp @e[tag=dw_cart,limit=1] ${R.x1 + 2.5} ${FLOOR_Y + 1.1} ${R.z + 0.5}`)
  await sleep(800)
  await rcon.run(`execute as @e[tag=dw_cart,limit=1] run data merge entity @s {Motion:[0.3d,0.0d,0.0d]}`)
  const t = now()
  const { v: v0 } = await readVehicle(bot, 'dw_cart')
  let last = v0
  while (now() - t < 12000) {
    const { v: vv, b: bb } = await readVehicle(bot, 'dw_cart')
    row('motionMerge', `cartPos=${vv?.map(f2)}`, `botPos=${bb?.map(f2)}`, `moved=${v0 && vv ? f2(dist2(v0, vv)) : '-'}`, `speedBlocksPerSec=${last && vv ? f2(dist2(last, vv) * 2) : '-'}`)
    last = vv
    await sleep(500)
  }
  await rcon.run(`tp @e[tag=dw_cart,limit=1] ${R.x1 + 2.5} ${FLOOR_Y + 1.1} ${R.z + 0.5}`)
  await sleep(1000)
  const { v: v3, b: b3 } = await readVehicle(bot, 'dw_cart')
  row('tpVehicle', `cartPos=${v3?.map(f2)}`, `botPosOnServer=${b3?.map(f2)}`, await rideState(bot))
  begin('T6-minecart-dismount', 'the bot dismounts: mineflayer\'s dismount (jump), then a sneak input')
  row(await tryDismount(bot))
  row(`botPosAfter=${(await botPos())?.map(f2)}`)
  begin('T6-minecart-ride-command', '`ride` mounts and dismounts the bot into the cart')
  const m = await rcon.probe(`ride ${BOT} mount @e[tag=dw_cart,limit=1]`)
  await sleep(800)
  row(`mountReply=${m}`, await rideState(bot))
  const dm = await rcon.probe(`ride ${BOT} dismount`)
  await sleep(800)
  row(`dismountReply=${dm}`, await rideState(bot))
  await rcon.probe('kill @e[tag=dw_cart]')
  await rcon.probe(`fill ${R.x1} ${FLOOR_Y + 1} ${R.z} ${R.x2} ${FLOOR_Y + 1} ${R.z} minecraft:air`)
  await rcon.probe(`fill ${R.x1 + 6} ${FLOOR_Y} ${R.z} ${R.x1 + 8} ${FLOOR_Y} ${R.z} minecraft:stone`)
}

// ---- T7: a box selector over bodies ---------------------------------------------
async function t7(bot) {
  await home(bot)
  await clearBodies()
  begin('T7-census', 'a pen box x 230..236, z 200..204, y 100..103 (dx=6,dy=3,dz=4); goats NoAI, summoned at stated centres; the selector\'s count read')
  const box = 'x=230,y=100,z=200,dx=6,dy=3,dz=4'
  const put = async (tag, x, z) => {
    await rcon.run(`summon minecraft:goat ${x} ${FLOOR_Y + 1} ${z} {NoAI:1b,Silent:1b,PersistenceRequired:1b,Tags:["dw_pen","${tag}"]}`)
    await sleep(200)
  }
  await put('a', 231.5, 201.5)
  await put('b', 233.5, 202.5)
  await put('c', 235.5, 203.5)
  row('threeInside', `count=${await countOf(`@e[tag=dw_pen,${box}]`)}`)
  await put('edge1', 237.3, 202.5)
  row('fourthCentre237.3(hitbox 236.85..237.75)', `count=${await countOf(`@e[tag=dw_pen,${box}]`)}`, `edgeOnly=${await countOf(`@e[tag=edge1,${box}]`)}`)
  await rcon.probe('kill @e[tag=edge1]')
  await put('edge2', 237.6, 202.5)
  row('fourthCentre237.6(hitbox 237.15..238.05)', `count=${await countOf(`@e[tag=dw_pen,${box}]`)}`, `edgeOnly=${await countOf(`@e[tag=edge2,${box}]`)}`)
  await rcon.probe('kill @e[tag=edge2]')
  await put('edge3', 229.7, 202.5)
  row('fourthCentre229.7(hitbox 229.25..230.15)', `count=${await countOf(`@e[tag=dw_pen,${box}]`)}`, `edgeOnly=${await countOf(`@e[tag=edge3,${box}]`)}`)
  await rcon.probe('kill @e[tag=edge3]')
  await put('edge4', 229.4, 202.5)
  row('fourthCentre229.4(hitbox 228.95..229.85)', `count=${await countOf(`@e[tag=dw_pen,${box}]`)}`, `edgeOnly=${await countOf(`@e[tag=edge4,${box}]`)}`)
  await rcon.probe('kill @e[tag=edge4]')
  await rcon.run(`execute store result score #pen ${OBJ} if entity @e[tag=dw_pen,${box}]`)
  row(`storeResultScore=${await score('#pen')}`)
  // The harness's runtime method under the ruling: teleport the animals in, then count.
  await rcon.run('tp @e[tag=a] 250.5 101 250.5')
  row('oneTeleportedOut', `count=${await countOf(`@e[tag=dw_pen,${box}]`)}`)
  await rcon.run('tp @e[tag=a] 232.5 101 203.5')
  row('teleportedBackIn', `count=${await countOf(`@e[tag=dw_pen,${box}]`)}`)
  await rcon.probe('kill @e[tag=dw_pen]')
}

// ---- T8: effect and damage aimed at a mob --------------------------------------------
async function t8(bot) {
  await home(bot)
  await clearBodies()
  begin('T8-mob-effect-damage', 'a NoAI pig (200 health) 5 east; `effect give` glowing, `damage` plain, `damage … by <bot>`; health, effects and position read')
  const D = { x: H.x + 5.5, y: H.y, z: H.z + 0.5 }
  const before = await summonDummy(D, 'dw_fx')
  row(`effectGive=${await rcon.probe('effect give @e[tag=dw_fx] minecraft:glowing 10 0 true')}`)
  await sleep(300)
  row(`active_effects=${await entityGet('@e[tag=dw_fx,limit=1]', 'active_effects')}`)
  const p0 = parsePos(await entityGet('@e[tag=dw_fx,limit=1]', 'Pos'))
  row(`damagePlain=${await rcon.probe('damage @e[tag=dw_fx,limit=1] 3 minecraft:generic')}`)
  await sleep(300)
  const h1 = parseNum(await entityGet('@e[tag=dw_fx,limit=1]', 'Health'))
  const p1 = parsePos(await entityGet('@e[tag=dw_fx,limit=1]', 'Pos'))
  row(`healthBefore=${before}`, `healthAfterPlain=${h1}`, `movedAfterPlain=${p0 && p1 ? f2(dist2(p0, p1)) : '-'}`)
  row(`damageBy=${await rcon.probe(`damage @e[tag=dw_fx,limit=1] 3 minecraft:player_attack by ${BOT}`)}`)
  await sleep(600)
  const h2 = parseNum(await entityGet('@e[tag=dw_fx,limit=1]', 'Health'))
  const p2 = parsePos(await entityGet('@e[tag=dw_fx,limit=1]', 'Pos'))
  row(`healthAfterBy=${h2}`, `movedAfterBy=${p1 && p2 ? f2(dist2(p1, p2)) : '-'}`, '(NoAI: a body that does not move)')
  await rcon.probe('kill @e[tag=dw_fx]')
  // The same two blows on a body with its AI, which the knockback can move.
  await rcon.run(`summon minecraft:pig ${D.x} ${D.y} ${D.z} {Silent:1b,PersistenceRequired:1b,Tags:["dw_fx2"],attributes:[{id:"minecraft:max_health",base:200d},{id:"minecraft:movement_speed",base:0d}],Health:200f}`)
  await sleep(500)
  const q0 = parsePos(await entityGet('@e[tag=dw_fx2,limit=1]', 'Pos'))
  await rcon.probe('damage @e[tag=dw_fx2,limit=1] 3 minecraft:generic')
  await sleep(800)
  const q1 = parsePos(await entityGet('@e[tag=dw_fx2,limit=1]', 'Pos'))
  await rcon.probe(`damage @e[tag=dw_fx2,limit=1] 3 minecraft:player_attack by ${BOT}`)
  await sleep(800)
  const q2 = parsePos(await entityGet('@e[tag=dw_fx2,limit=1]', 'Pos'))
  row('aiBody', `movedAfterPlain=${q0 && q1 ? f2(dist2(q0, q1)) : '-'}`, `movedAfterBy=${q1 && q2 ? f2(dist2(q1, q2)) : '-'}`, `awayFromBot=${q1 && q2 ? f2(dist2(q2, [H.x, 0, H.z]) - dist2(q1, [H.x, 0, H.z])) : '-'}`)
  await rcon.probe('kill @e[tag=dw_fx2]')
}

// ---- T9: the spyglass ------------------------------------------------------------------
async function t9(bot) {
  await home(bot)
  await rcon.run(`gamemode survival ${BOT}`)
  begin('T9-spyglass', 'the bot raises a spyglass for 1.5 s; the `using_item` advancement\'s count read (it re-arms itself each tick)')
  await hold(bot, 'spyglass')
  await resetScore('#spyglass')
  await bot.lookAt(new Vec3(H.x + 30, H.y + 1, H.z), true)
  bot.activateItem()
  await sleep(1500)
  bot.deactivateItem()
  await sleep(600)
  row(`using_item_ticks=${await score('#spyglass')}`)
  await rcon.run(`clear ${BOT}`)
}

async function teardown(bot) {
  try {
    await clearBodies()
  } catch (e) {
    console.error(`teardown: ${e.message}`)
  }
  try {
    for (let x = H.x - PAD; x <= H.x + PAD; x += 20) {
      await rcon.probe(`fill ${x} ${FLOOR_Y - 1} ${H.z - PAD} ${Math.min(x + 19, H.x + PAD)} ${FLOOR_Y + 8} ${H.z + PAD} minecraft:air`)
    }
    await rcon.probe('forceload remove all')
    await execFileP('docker', ['exec', container, 'rm', '-rf', PACK])
  } catch (e) {
    console.error(`teardown: ${e.message}`)
  }
  try { bot?.quit('probe done') } catch { /* gone */ }
}

let bot
try {
  console.log(`== festival probe: ${await rcon.probe('version')} ==`)
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
