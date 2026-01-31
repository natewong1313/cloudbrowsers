import type { apiWorker } from "alchemy.run";
import { DurableObject } from "cloudflare:workers";
import pino from "pino";
import {
  ContainerFetchError,
  NoCapacityError,
  type NewSessionDetails,
} from "./browser-container";
import { newBrowserContainerId, type BrowserContainerId } from "./shared";

// per region
export class BrowserContainerRouter extends DurableObject {
  declare env: typeof apiWorker.Env;
  region!: string; // wnam, enam, etc
  logger!: pino.Logger;
  containerCapacity = new Map<BrowserContainerId, number>();

  // safe to call this multiple times
  init(region: string) {
    this.region = region;
    this.logger = pino({ level: "debug" }).child({
      module: "BrowserContainerRouter",
      region,
    });
  }

  async requestSession(): Promise<NewSessionDetails> {
    const id = newBrowserContainerId();
    const stub = this.env.BROWSER_CONTAINER.getByName(id);
    await stub.init(id, this.region);

    const sessionResult = await stub.newSession();
    // clean this up later
    if (
      sessionResult instanceof NoCapacityError ||
      sessionResult instanceof ContainerFetchError
    ) {
      throw new Error(sessionResult.message);
    }
    return sessionResult;
  }

  // Called by child Containers
  updateCapacity(containerId: BrowserContainerId, capacity: number) {
    this.containerCapacity.set(containerId, capacity);
  }
}
