import { Container } from "@cloudflare/containers";

export type BrowserContainerId = string & { __brand: "BrowserContainerId" };
export const BROWSER_CONTAINER_WS_PORT = 6700;

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

  async init(id: BrowserContainerId) {
    this.id = id;
    await this.startAndWaitForPorts();
  }
}
