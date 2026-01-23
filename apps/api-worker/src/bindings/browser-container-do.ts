import { DurableObject, env } from "cloudflare:workers";
import { getContainer, switchPort } from "@cloudflare/containers";
import {
  newBrowserContainerId,
  type BrowserContainer,
} from "./browser-container";

export type BrowserSessionId = string & { __brand: "BrowserSessionId" };

export const newBrowserSessionId = (): BrowserSessionId => {
  return crypto.randomUUID() as BrowserSessionId;
};

export class BrowserContainerDurableObject extends DurableObject {
  container!: DurableObjectStub<BrowserContainer>;

  async init() {
    const containerId = newBrowserContainerId();
    this.container = getContainer(env.BROWSER_CONTAINER, containerId);

    console.log("starting container", containerId);
    await this.container.init(containerId);
    console.log("started container");
  }

  async newSession(sessionId: BrowserSessionId) {
    console.log("recv container do req", sessionId);
    const resp = await this.container.containerFetch("/");
  }
}
