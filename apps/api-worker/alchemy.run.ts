import type { BrowserContainer } from "@/bindings/browser-container";
import type { BrowserContainerRouter } from "@/bindings/browser-container-router";
import alchemy from "alchemy";
import { Worker, Container, DurableObjectNamespace } from "alchemy/cloudflare";

const app = await alchemy("cloudbrowsers-api-worker");

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

const browserContainerRouter = DurableObjectNamespace<BrowserContainerRouter>(
  "browser-container-router",
  {
    className: "BrowserContainerRouter",
    sqlite: false,
  },
);

export const API_WORKER_PORT = 7000;
export const apiWorker = await Worker("api-worker", {
  entrypoint: "src/index.ts",
  compatibility: "node",
  bindings: {
    BROWSER_CONTAINER: browserContainer,
    BROWSER_CONTAINER_ROUTER: browserContainerRouter,
  },
  dev: {
    port: API_WORKER_PORT,
  },
});

await app.finalize();
