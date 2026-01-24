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
  private reconnectAttempts = 0;

  /**
   * Creates and starts the underlying container, then opens the websocket connection
   */
  async init() {
    const start = performance.now();
    const containerId = newBrowserContainerId();
    this.container = getContainer(env.BROWSER_CONTAINER, containerId);

    await this.container.init(containerId);
    this.connectToInternalContainerWS(); // this.ctx.waitUntil(this.connectToInternalContainerWS());
    console.log("initialized container in", performance.now() - start, "ms");
  }

  private async connectToInternalContainerWS() {
    // Create a WebSocket upgrade request targeting the container's /ws endpoint
    const wsRequest = switchPort(
      new Request("http://container/ws", {
        headers: {
          Upgrade: "websocket",
        },
      }),
      BROWSER_CONTAINER_WS_PORT,
    );

    console.log(
      "Connecting to container WebSocket on port",
      BROWSER_CONTAINER_WS_PORT,
    );

    try {
      const response = await this.container.fetch(wsRequest);

      this.ws = response.webSocket;
      if (!this.ws) {
        console.error("Expected WebSocket response but got HTTP response");
        this.scheduleReconnect();
        return;
      }

      this.ws.accept();

      this.ws.addEventListener("open", () => {
        console.log("Connected to container WebSocket");
        this.reconnectAttempts = 0;
      });

      this.ws.addEventListener("message", (event) => {
        console.log("Container WS message:", event.data);
      });

      this.ws.addEventListener("close", () => {
        console.log("Container WS closed, scheduling reconnect...");
        this.scheduleReconnect();
      });

      this.ws.addEventListener("error", (error) => {
        console.error("Container WS error:", error);
      });
    } catch (error) {
      console.error("Failed to connect to container WebSocket:", error);
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect() {
    const delay = Math.min(1000 * 2 ** this.reconnectAttempts, 30000);
    this.reconnectAttempts++;
    console.log(
      `Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`,
    );
    setTimeout(() => this.connectToInternalContainerWS(), delay);
  }

  async newSession() {
    console.log("creating new session");
  }
}
