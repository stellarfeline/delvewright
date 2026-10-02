// Which reply belongs to which command, for the harness's chat channel.
//
// The harness speaks to the server as an opped player, and the server's answer to
// a command arrives as an ordinary chat line that names nothing it answers. More
// than one command can be in flight at once: the main sequence issues one while a
// body that hit the bot is being staged away, or a walk leg's health is being
// restored, from an event handler. A reader that takes "every line since I sent
// mine" as its reply reads the other command's answer as its own — measured on
// the vesperhold release ladder, where a staged blow on a body already dead was
// refused, and the wave's strike, sent a moment earlier, reported that refusal as
// its own and failed the run.
//
// So a command that is read is sent BRACKETED: an opening marker, the command, a
// closing marker, written to the socket back to back in one synchronous turn. The
// server executes one player's commands in the order they arrive and answers each
// before it reads the next, so the lines between this command's two markers are
// its answer and nobody else's. Wall time decides nothing here.

/** The markers of one bracketed command. */
export interface Bracket {
  readonly serial: number;
  /** The line the server echoes before the command's answer. */
  readonly open: string;
  /** The line the server echoes after it. */
  readonly close: string;
}

/** What the server said between one command's markers. */
export interface BracketedReply {
  /** Every line between the markers, in arrival order. */
  readonly lines: readonly string[];
  /** False when the closing marker never arrived: the reply was not observed. */
  readonly answered: boolean;
}

const MARKER = /^\[dw:cmd (\d+) (open|close)\]$/;

/** The marker line for `serial`. */
export function bracketMarker(serial: number, edge: "open" | "close"): string {
  return `[dw:cmd ${serial} ${edge}]`;
}

/** The command that makes the server echo `line` back to the bot alone. */
export function echoCommand(line: string): string {
  return `/tellraw @s ${JSON.stringify({ text: line })}`;
}

/**
 * The receive side of the bracket protocol: fed every chat line in arrival order,
 * it files the lines between a bracket's markers under that bracket.
 */
export class ReplyBrackets {
  private serial = 0;
  /** The bracket whose opening marker has arrived and closing marker has not. */
  private current: { serial: number; lines: string[] } | undefined;
  private readonly closed = new Map<number, string[]>();
  private readonly waiting = new Map<number, (reply: BracketedReply) => void>();

  /** Allocate the next bracket. */
  begin(): Bracket {
    this.serial += 1;
    return {
      serial: this.serial,
      open: bracketMarker(this.serial, "open"),
      close: bracketMarker(this.serial, "close"),
    };
  }

  /**
   * Feed one chat line. True when the line was a bracket marker — protocol, not
   * speech, so the caller keeps it out of whatever else reads the chat.
   */
  observe(line: string): boolean {
    const m = MARKER.exec(line.trim());
    if (!m) {
      this.current?.lines.push(line);
      return false;
    }
    const serial = Number(m[1]);
    if (m[2] === "open") {
      this.current = { serial, lines: [] };
      return true;
    }
    const lines = this.current?.serial === serial ? this.current.lines : [];
    this.current = undefined;
    const resolve = this.waiting.get(serial);
    if (resolve) {
      this.waiting.delete(serial);
      resolve({ lines, answered: true });
    } else {
      this.closed.set(serial, lines);
    }
    return true;
  }

  /** The reply to `bracket`, once its closing marker arrives or `timeoutMs` passes. */
  reply(bracket: Bracket, timeoutMs: number): Promise<BracketedReply> {
    const done = this.closed.get(bracket.serial);
    if (done !== undefined) {
      this.closed.delete(bracket.serial);
      return Promise.resolve({ lines: done, answered: true });
    }
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.waiting.delete(bracket.serial);
        resolve({ lines: [], answered: false });
      }, timeoutMs);
      this.waiting.set(bracket.serial, (reply) => {
        clearTimeout(timer);
        resolve(reply);
      });
    });
  }
}
