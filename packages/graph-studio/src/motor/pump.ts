/** Requests answered one at a time, in the order they arrived. */
import type { Envelope, Request, Result } from "./protocol.ts";
import type { ForceHost } from "./liveLoop.ts";
import { serve } from "./serve.ts";
import type { Session } from "./session.ts";

export type Reply = (message: Envelope<Result>, transfer: ArrayBufferLike[]) => void;

export function createPump(session: Session, reply: Reply, forces: ForceHost | null = null): (message: Envelope<Request>) => void {
  let queue: Promise<void> = Promise.resolve();
  return (message) => {
    queue = queue.then(async () => {
      const answer = await serve(session, message.body, forces);
      reply({ seq: message.seq, body: answer.result }, answer.transfer);
    });
  };
}
