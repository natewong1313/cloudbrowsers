import { Container, switchPort } from "@cloudflare/containers";
import type { apiWorker } from "alchemy.run";
import pino from "pino";
import type { BrowserContainerId } from "./shared";

export type NewSessionDetails = {
  sessionId: string;
  wsConnectPath: string;
};

export const BROWSER_CONTAINER_WS_PORT = 6700;
export class BrowserContainer extends Container {
  declare env: typeof apiWorker.Env;
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
  region!: string;
  logger!: pino.Logger;
  capacity = 0;

  async init(
    id: BrowserContainerId,
    region: string,
  ): Promise<void | FailedToInitializeError> {
    const start = new Date().getTime();
    this.id = id;
    this.region = region;
    this.logger = pino({ level: "debug" }).child({
      module: "BrowserContainer",
      id,
    });

    this.logger.info("Starting container");

    try {
      await this.start();
      await this.waitForPort({
        portToCheck: BROWSER_CONTAINER_WS_PORT,
        retries: 100,
        waitInterval: 10,
      });
    } catch (e) {
      return new FailedToInitializeError("waiting for container port failed", {
        cause: e,
      });
    }

    const result = await this.establishContainerCapacityWsConnection();
    if (result instanceof ContainerWebsocketError) {
      return new FailedToInitializeError("ws failed", { cause: result });
    }

    this.logger.debug(
      `Finished starting container in ${new Date().getTime() - start}ms`,
    );
  }

  async newSession(): Promise<
    NewSessionDetails | NoCapacityError | ContainerFetchError
  > {
    this.logger.info("Processing new session request");
    if (this.capacity === 0) {
      return new NoCapacityError();
    }
    // We can optimistically reserve capacity
    this.capacity--;

    let response: Response;
    try {
      response = await this.fetch(
        switchPort(
          new Request("http://container/new", {
            method: "POST",
          }),
          BROWSER_CONTAINER_WS_PORT,
        ),
      );
    } catch (e) {
      return new ContainerFetchError("failed to fetch /new", { cause: e });
    }
    if (!response.ok) {
      this.capacity++;
      return new ContainerFetchError(response.statusText);
    }

    const { id: sessionId } = (await response.json()) as { id: string };
    const wsConnectPath = `/session/${this.id}/${sessionId}`;

    this.logger.debug({ sessionId, wsConnectPath }, "new session created");
    return { sessionId, wsConnectPath };
  }

  /**
   * Connects to the internal containers capacity websocket route
   * Then waits for it to send over its capacity
   */
  private async establishContainerCapacityWsConnection(): Promise<void | ContainerWebsocketError> {
    this.logger.info("Connecting to capacity websocket");

    let response: Response;
    try {
      response = await this.fetch(
        switchPort(
          new Request("http://container/capacity", {
            headers: {
              Upgrade: "websocket",
            },
          }),
          BROWSER_CONTAINER_WS_PORT,
        ),
      );
    } catch (e) {
      return new ContainerWebsocketError("failed to fetch websocket", {
        cause: e,
      });
    }

    const ws = response.webSocket;
    if (!ws) {
      return new ContainerWebsocketError("no websocket");
    }
    ws.accept();

    ws.addEventListener("message", (e) => this.onCapacityUpdate(e));
    ws.addEventListener("error", ({ error }) => {
      this.logger.warn({ error }, "Error from capacity websocket");
    });
    ws.addEventListener("close", () => {
      this.logger.warn("Capacity websocket connection closed");
    });

    // Wait for a capacity message before continuing
    return new Promise((resolve) => {
      ws.addEventListener("message", () => resolve(), { once: true });
    });
  }

  private async onCapacityUpdate(e: MessageEvent) {
    this.logger.debug({ capacity: this.capacity }, "New capacity update");
    this.capacity = parseInt(e.data);

    this.logger.debug(
      { capacity: this.capacity },
      "Sending capacity update to parent",
    );
    const parentDO = this.env.BROWSER_CONTAINER_ROUTER.getByName(this.region);
    await parentDO.updateCapacity(this.id, this.capacity);
  }
}

export class FailedToInitializeError extends Error {
  constructor(msg: string, opts?: ErrorOptions) {
    super(msg, opts);
    this.name = "FailedToInitializeError";
  }
}

export class NoCapacityError extends Error {
  constructor() {
    super();
    this.name = "NoCapacityError";
  }
}

export class ContainerFetchError extends Error {
  constructor(msg: string, opts?: ErrorOptions) {
    super(msg, opts);
    this.name = "ContainerFetchError";
  }
}

class ContainerWebsocketError extends Error {
  constructor(msg: string, opts?: ErrorOptions) {
    super(msg, opts);
    this.name = "ContainerWebsocketError";
  }
}
