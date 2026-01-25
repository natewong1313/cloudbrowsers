import { DurableObject, env } from "cloudflare:workers";
import { getContainer, switchPort } from "@cloudflare/containers";
import {
  BROWSER_CONTAINER_WS_PORT,
  newBrowserContainerId,
  type BrowserContainer,
  type BrowserContainerId,
} from "./browser-container";

//TODO: we should autogen this from rust types
export type BrowserContainerState = {
  size: number;
};
type NewBrowserSession = {
  id: string;
  ws_addr: string;
};

/**
 * Sidecar durable object alongside the Container which itself is a sidecar to the actual container...
 */
export class BrowserContainerDurableObject extends DurableObject {
  container!: DurableObjectStub<BrowserContainer>;
  private containerId!: BrowserContainerId;
  private ws: WebSocket | null = null;
  // save container state in memory. kv/sql doesnt make sense for now
  private containerState: BrowserContainerState = { size: 0 };

  private log(...msg: any[]) {
    console.log("[BrowserContainerDurableObject]", ...msg);
  }
  /**
   * Creates and starts the underlying container, then opens the websocket connection
   */
  async init() {
    const start = performance.now();

    this.containerId = newBrowserContainerId();
    this.log("getting container", this.containerId);
    this.container = getContainer(env.BROWSER_CONTAINER, this.containerId);

    this.log("initializing container", this.containerId);
    await this.container.init(this.containerId);
    this.log("initialized container in", performance.now() - start, "ms");

    this.log("connecting to container websocket", this.containerId);
    await this.connectToInternalContainerWS(); // this.ctx.waitUntil(this.connectToInternalContainerWS());
  }

  /**
   * The container exposes a /state websocket route to broadcasts its state changes
   * this helps us stay in sync with its capacity
   * TODO: maybe rename route to /capacity
   */
  private async connectToInternalContainerWS() {
    const wsRequest = switchPort(
      new Request("http://container/state", {
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
    this.ws?.accept();
    console.log("ACCEPTED");
    this.ws.addEventListener("message", (e) =>
      this.handleMessageFromContainer(e),
    );
    this.ws.addEventListener("error", ({ error }) => {
      this.log("container ws error", error);
    });
    this.ws.addEventListener("close", () => {
      this.log("closed connection?");
    });
  }

  private async handleMessageFromContainer({ data }: MessageEvent) {
    this.log("new message from container", data);
    // TODO: validate that the message is valid since we are mutating state
    const parsed = JSON.parse(data) as BrowserContainerState;
    this.containerState = parsed;
    // TODO: Use containerState for capacity management
  }

  async newSession() {
    this.log("requesting new browser session");
    const newRequest = switchPort(
      new Request("http://container/new", {
        method: "POST",
      }),
      BROWSER_CONTAINER_WS_PORT,
    );
    const response = await this.container.fetch(newRequest);

    if (!response.ok) {
      throw new Error(`Failed to create new session: ${response.statusText}`);
    }

    const { id: sessionId } = (await response.json()) as NewBrowserSession;
    this.log("new session created:", sessionId);

    // Return both the underlying container id and the session id so we can route correctly
    const wsConnectPath = `/session/${this.containerId}/${sessionId}`;
    this.log("WebSocket connect path:", wsConnectPath);
    return { sessionId, wsConnectPath };
  }

  // /**
  //  * Pass through to container instance
  //  */
  // async fetch(request: Request) {
  //   const url = new URL(request.url);
  //   // Handle WebSocket proxy requests to browser sessions
  //   if (url.pathname.startsWith("/session/")) {
  //     this.log("proxying websocket request to container");
  //     const proxyRequest = switchPort(request, BROWSER_CONTAINER_WS_PORT);
  //     return this.container.fetch(proxyRequest);
  //   }
  //   this.log("invalid fetch request", url.pathname);
  //   return new Response("Not Found", { status: 404 });
  // }
}
