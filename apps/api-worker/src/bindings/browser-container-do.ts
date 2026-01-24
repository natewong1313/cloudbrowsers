import { DurableObject, env } from "cloudflare:workers";
import { getContainer, switchPort } from "@cloudflare/containers";
import {
  BROWSER_CONTAINER_WS_PORT,
  newBrowserContainerId,
  type BrowserContainer,
} from "./browser-container";

/**
 * Sidecar durable object alongside the Container which itself is a sidecar to the actual container...
 */
export class BrowserContainerDurableObject extends DurableObject {
  container!: DurableObjectStub<BrowserContainer>;
  private ws: WebSocket | null = null;

  private log(...msg: any[]) {
    console.log("[BrowserContainerDurableObject]", ...msg);
  }
  /**
   * Creates and starts the underlying container, then opens the websocket connection
   */
  async init() {
    const start = performance.now();

    const containerId = newBrowserContainerId();
    this.log("getting container", containerId);
    this.container = getContainer(env.BROWSER_CONTAINER, containerId);

    this.log("initializing container", containerId);
    await this.container.init(containerId);
    this.log("connecting to container websocket", containerId);
    this.connectToInternalContainerWS(); // this.ctx.waitUntil(this.connectToInternalContainerWS());
    this.log("initialized container in", performance.now() - start, "ms");
  }

  private async connectToInternalContainerWS() {
    const wsRequest = switchPort(
      new Request("http://container/ws", {
        headers: {
          Upgrade: "websocket",
        },
      }),
      BROWSER_CONTAINER_WS_PORT,
    );

    this.log("connecting to container ws at", BROWSER_CONTAINER_WS_PORT);

    const response = await this.container.fetch(wsRequest);
    this.ws = response.webSocket;
    if (!this.ws) {
      this.log("failed to connect to websocket");
      return;
    }
    this.ws.addEventListener("message", (event) => {
      this.log("recv message", event.data);
    });
    this.ws.addEventListener("error", ({ error }) => {
      this.log("container ws error", error);
    });
  }

  async newSession() {
    console.log("creating new session");
  }
}
