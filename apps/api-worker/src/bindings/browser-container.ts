import { Container, switchPort } from "@cloudflare/containers";
import pino from "pino";

export const BROWSER_CONTAINER_WS_PORT = 6700;
export type BrowserContainerId = string & { __brand: "BrowserContainerId" };

export type BrowserSessionDetails = {
  sessionId: string;
  wsConnectPath: string;
};

export const newBrowserContainerId = (): BrowserContainerId => {
  return crypto.randomUUID() as BrowserContainerId;
};

export class BrowserContainer extends Container {
  // Port the container listens on (default: 8080)
  defaultPort = BROWSER_CONTAINER_WS_PORT;
  // Ports that must be ready during container startup
  requiredPorts = [BROWSER_CONTAINER_WS_PORT];
  // Time before container sleeps due to inactivity (default: 30s)
  sleepAfter = "5m";
  // Environment variables passed to the container
  // envVars = {
  //   MESSAGE: "I was passed in via the container class!",
  // };
  id!: BrowserContainerId;
  logger!: pino.Logger;
  capacity = 0;

  async init(id: BrowserContainerId) {
    const start = new Date().getTime();
    this.id = id;
    this.logger = pino({ level: "debug" }).child({
      module: "BrowserContainer",
      id: id,
    });

    this.logger.info("Starting container");

    await this.start();
    await this.waitForPort({
      portToCheck: BROWSER_CONTAINER_WS_PORT,
      retries: 100,
      waitInterval: 10,
    });
    await this.establishContainerCapacityWsConnection();

    this.logger.debug(
      `Finished starting container in ${new Date().getTime() - start}ms`,
    );
  }

  async newSession(): Promise<BrowserSessionDetails> {
    this.logger.info("Processing new session request");
    if (this.capacity === 0) {
      throw new Error("no capacity");
    }
    // We can optimistically reserve capacity
    this.capacity--;

    const response = await this.fetch(
      switchPort(
        new Request("http://container/new", {
          method: "POST",
        }),
        BROWSER_CONTAINER_WS_PORT,
      ),
    );
    if (!response.ok) {
      this.capacity++;
      throw new Error(`Failed to create new session: ${response.statusText}`);
    }

    const { id: sessionId } = (await response.json()) as { id: string };
    this.logger.debug({ sessionId }, "new session created");

    // Return both the underlying container id and the session id so we can route correctly
    const wsConnectPath = `/session/${this.id}/${sessionId}`;
    return { sessionId, wsConnectPath };
  }

  private async establishContainerCapacityWsConnection() {
    this.logger.info("Connecting to capacity websocket");
    const response = await this.fetch(
      switchPort(
        new Request("http://container/capacity", {
          headers: {
            Upgrade: "websocket",
          },
        }),
        BROWSER_CONTAINER_WS_PORT,
      ),
    );
    const ws = response.webSocket;
    if (!ws) {
      this.logger.warn("Failed to establish container websocket connection");
      return;
    }
    ws.accept();
    ws.addEventListener("message", (e) => this.handleCapacityUpdate(e));
    ws.addEventListener("error", ({ error }) => {
      this.logger.warn({ error }, "Error from capacity websocket");
    });
    ws.addEventListener("close", () => {
      this.logger.warn("Capacity websocket connection closed");
    });
  }

  // Data type is json incase we add more fields
  private async handleCapacityUpdate({ data }: MessageEvent) {
    this.capacity = parseInt(data);
    this.logger.debug({ capacity: this.capacity }, "Recv capacity update");
  }
}
