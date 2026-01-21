import type {
  containersQueue,
  containersQueueConsumerWorker,
} from "alchemy.run";

/**
 * Worker that consumes queue messages every 250 ms
 */
export default {
  async queue(
    batch: typeof containersQueue.Batch,
    _env: typeof containersQueueConsumerWorker.Env,
  ) {
    // TODO: check if this will get backed up and what happens
    // if we have way more requests than active browsers
    for (const message of batch.messages) {
      console.log(message);
      message.ack();
    }
    batch.ackAll();
  },
};
