// The player half of `probe-click-attribution.py`: a bot that joins the probe
// server and, on command, attacks or right-clicks one entity.
//
// It is driven over stdin, one act per line, and answers on stdout, one line per
// act, so the Python driver can read the server's own records between acts.
//
//   attack <entity-name> <x> <y> <z>   left-click the nearest `<entity-name>`
//                                      (mineflayer's `entity.name`, e.g. `zombie`,
//                                      `interaction`) within 1.5 blocks of the point
//   use    <entity-name> <x> <y> <z>   right-click it
//   census                             how many entities besides itself the bot sees
//   quit                               leave
//
// Replies: `joined <username>` once spawned, `entities <n>` to a census, `done <act>` after an act,
// `error <reason>` when the named entity is not where the driver said it is.
//
// The bot is a client and nothing else: it holds no expectation about what the
// server records. Run from a directory whose `node_modules` carries mineflayer at
// the harness's pinned version (`harness/package.json`); node resolves a bare
// import from the importing FILE's directory upward, so the driver copies this
// file beside that `node_modules` before running it.

import { createInterface } from 'node:readline'
import mineflayer from 'mineflayer'

const [, , host, port, username] = process.argv
if (!host || !port || !username) {
  console.error('usage: node probe-click-attribution.mjs <host> <port> <username>')
  process.exit(2)
}

const bot = mineflayer.createBot({
  host,
  port: Number(port),
  username,
  auth: 'offline',
  version: '1.21.11',
})

bot.once('spawn', () => {
  console.log(`joined ${bot.username}`)
})
bot.on('entitySpawn', (e) => {
  console.error(`seen ${e.name} at ${e.position}`)
})

bot.on('kicked', (reason) => {
  console.log(`error kicked ${JSON.stringify(reason)}`)
  process.exit(1)
})
bot.on('error', (err) => {
  console.log(`error ${err.message}`)
  process.exit(1)
})

function nearest(name, x, y, z) {
  let best = null
  let bestD = Infinity
  for (const e of Object.values(bot.entities)) {
    if (e === bot.entity || e.name !== name) continue
    const d = Math.hypot(e.position.x - x, e.position.y - y, e.position.z - z)
    if (d < bestD) {
      best = e
      bestD = d
    }
  }
  return bestD <= 1.5 ? best : null
}

const lines = createInterface({ input: process.stdin })
lines.on('line', async (line) => {
  const [act, name, xs, ys, zs] = line.trim().split(/\s+/)
  if (act === 'quit') {
    bot.quit()
    process.exit(0)
  }
  if (act === 'census') {
    console.log(`entities ${Object.values(bot.entities).filter((e) => e !== bot.entity).length}`)
    return
  }
  const [x, y, z] = [xs, ys, zs].map(Number)
  const target = nearest(name, x, y, z)
  if (!target) {
    const seen = Object.values(bot.entities)
      .filter((e) => e !== bot.entity)
      .map((e) => `${e.name}@${e.position.x.toFixed(1)},${e.position.y.toFixed(1)},${e.position.z.toFixed(1)}`)
    console.log(`error no ${name} within 1.5 of ${x},${y},${z}; seen: ${seen.join(' ') || 'nothing'}`)
    return
  }
  await bot.lookAt(target.position.offset(0, 1, 0), true)
  if (act === 'attack') {
    bot.attack(target)
  } else if (act === 'use') {
    await bot.activateEntity(target)
  } else {
    console.log(`error unknown act ${act}`)
    return
  }
  console.log(`done ${act} ${name}`)
})
