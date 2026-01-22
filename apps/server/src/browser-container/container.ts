import { Container } from "@cloudflare/containers";
import type { InstanceType } from "alchemy/cloudflare";

export type BrowserContainerId = string & { __brand: "BrowserContainerId" };

export const BROWSER_CONTAINER_PORT = 6700;
export const BROWSER_CONTAINER_INSTANCE_TYPE: InstanceType = "basic";

export class BrowserContainer extends Container {
  // Port the container listens on (default: 8080)
  defaultPort = BROWSER_CONTAINER_PORT;
  // Time before container sleeps due to inactivity (default: 30s)
  sleepAfter = "2m";
  // Environment variables passed to the container
  // envVars = {
  //   MESSAGE: "I was passed in via the container class!",
  // };
}
