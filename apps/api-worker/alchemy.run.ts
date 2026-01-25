import type { BrowserContainer } from "@/bindings/browser-container";
import type { BrowserContainerDurableObject } from "@/bindings/browser-container-do";
import alchemy from "alchemy";
import { Worker, DurableObjectNamespace, Container } from "alchemy/cloudflare";

const app = await alchemy("cloudbrowsers-api-worker");

const browserContainerDurableObject =
  DurableObjectNamespace<BrowserContainerDurableObject>(
    "browser-container-do",
    {
      className: "BrowserContainerDurableObject",
      sqlite: true,
    },
  );
const browserContainer = await Container<BrowserContainer>(
  "browser-container",
  {
    className: "BrowserContainer",
    adopt: true,
    build: {
      context: `${import.meta.dirname}/../browser-container`,
      dockerfile: "Dockerfile",
      // args: {
      //   IMAGE_VERSION: "1.24-alpine",
      // },
    },
  },
);

export const API_WORKER_PORT = 7000;
export const apiWorker = await Worker("api-worker", {
  entrypoint: "src/index.ts",
  compatibility: "node",
  bindings: {
    BROWSER_CONTAINER_DO: browserContainerDurableObject,
    BROWSER_CONTAINER: browserContainer,
  },
  dev: {
    port: API_WORKER_PORT,
  },
});

await app.finalize();
