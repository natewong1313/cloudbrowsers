import {
  BROWSER_CONTAINER_PORT,
  BrowserContainer,
  type BrowserContainerId,
} from "@/browser-container/container";
import { getContainer, switchPort } from "@cloudflare/containers";
import { DurableObject, env } from "cloudflare:workers";

/**
 * Sidecar Durable Object to the BrowserContainer
 * By coupling a durable object to the container, we can ** keep the container in the same colo
 */
export class BrowserContainerSidecar extends DurableObject {
  private containerId!: BrowserContainerId;

  async fetch(req: Request): Promise<Response> {
    if (req.headers.get("upgrade") !== "websocket") {
      return new Response("Not a websocket request", { status: 400 });
    }

    // Forward the websocket request to the underlying container
    const container = getContainer(env.BROWSER_CONTAINER, "testcontainer");
    const newReq = switchPort(req, 6700);
    console.log("NEW REQ", newReq.url);
    return await container.fetch(newReq);
  }

  /**
   * Sets up the underlying container
   */
  async init(containerId: BrowserContainerId) {
    console.log("called setup");
    this.containerId = containerId;

    console.log("Getting container");
    const container = getContainer(env.BROWSER_CONTAINER, containerId);
    console.log("Starting container");
    // await container.start();
    console.log("Started container");
  }

  /**
   * Handle fetch requests directly (for WebSocket upgrade which can't go through RPC)
   */
  async fetchOld(req: Request): Promise<Response> {
    if (req.headers.get("upgrade") !== "websocket") {
      return new Response("missing upgrade header", { status: 400 });
    }

    const container = getContainer<BrowserContainer>(
      env.BROWSER_CONTAINER,
      this.containerId,
    );
    const containerState = await container.getState();

    const readyToAccept =
      containerState.status === "running" ||
      containerState.status === "healthy";
    if (!readyToAccept) {
      throw new Error("Container not running!");
    }
    return await container.fetch(switchPort(req, BROWSER_CONTAINER_PORT));
  }

  webSocketMessage(
    ws: WebSocket,
    message: string | ArrayBuffer,
  ): void | Promise<void> {
    console.log("message", message);
    ws.send(`Received message: ${message.toString()}`);
  }

  webSocketClose(
    _ws: WebSocket,
    code: number,
    reason: string,
  ): void | Promise<void> {
    console.log("close", code, reason);
  }

  webSocketError(_ws: WebSocket, error: unknown): void | Promise<void> {
    console.log("error", error);
  }
}
