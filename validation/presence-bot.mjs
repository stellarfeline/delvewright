// A player who is simply THERE — the one thing spec-0064's ladder needs and the
// critical-path harness cannot give it.
//
// The reset's event is "every reading of the online player count across a window
// was zero", so what has to be driven is presence itself: join, stay, leave on
// command, rejoin. `harness/src/run.ts` joins in order to WALK a delve and leaves
// when the walk ends; a run that finishes is a leave nobody scheduled, and a run
// that fails is a leave for the wrong reason. So this is its own program, and it
// does exactly one thing.
//
// It runs inside the ladder's compose project on the harness image, which already
// carries mineflayer at the pinned version:
//
//   docker compose -p <project> run -d --rm --no-deps \
//     -v <repo>/validation/presence-bot.mjs:/presence.mjs:ro \
//     --entrypoint node bot /presence.mjs
//
// Joined is announced on stdout, so the caller waits for the server's own answer
// (`list` over rcon) rather than for a sleep. SIGTERM — `docker stop` — is a
// clean quit: the leave is the player's, not a socket the server has to time out,
// which is the leave a host actually sees.
import mineflayer from 'mineflayer'

const host = process.env.DELVEWRIGHT_MC_HOST ?? 'server'
const port = Number(process.env.DELVEWRIGHT_MC_PORT ?? '25565')
const version = process.env.DELVEWRIGHT_MC_VERSION ?? '1.21.11'
const username = process.env.PRESENCE_USERNAME ?? 'dw-visitor'

const bot = mineflayer.createBot({ host, port, version, username, auth: 'offline' })

let quitting = false
const leave = () => {
  if (quitting) return
  quitting = true
  console.log('presence: leaving')
  try { bot.quit('presence: leaving') } catch { /* already gone */ }
  // The quit packet has to reach the server before the process does not exist.
  setTimeout(() => process.exit(0), 1500)
}
process.on('SIGTERM', leave)
process.on('SIGINT', leave)

bot.once('spawn', () => {
  console.log(`presence: ${bot.username} is in the world`)
})
bot.on('kicked', (reason) => {
  console.log(`presence: kicked ${typeof reason === 'string' ? reason : JSON.stringify(reason)}`)
  process.exit(quitting ? 0 : 3)
})
bot.on('error', (err) => {
  console.log(`presence: error ${err?.message ?? err}`)
  process.exit(quitting ? 0 : 4)
})
bot.on('end', () => {
  console.log('presence: connection ended')
  process.exit(quitting ? 0 : 5)
})
