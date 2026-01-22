import type { BrowserContainer } from "./src/browser-container/container";
import type { BrowserContainerSidecar } from "./src/durable-objects/browser-container-sidecar";
import alchemy from "alchemy";
import {
  Container,
  DurableObjectNamespace,
  KVNamespace,
  Queue,
  Worker,
  type Binding,
} from "alchemy/cloudflare";
import { D1Database } from "alchemy/cloudflare";
import { config } from "dotenv";
import { exit } from "node:process";

config({ path: "./.env" });

const app = await alchemy("cloudbrowsers");

const db = await D1Database("cloudbrowsers-db", {
  migrationsDir: "packages/db/src/migrations",
});

const kv = await KVNamespace("cloudbrowsers-kv");

export type QueueMessage = {
  userId: string;
};
export const containersQueue = await Queue<QueueMessage>(
  "cloudbrowsers-container-queue",
);
export const containersQueueConsumerWorker = await Worker(
  "cloudbrowsers-container-queue-consumer-worker",
  {
    entrypoint: "src/workers/queue-consumer.ts",
    eventSources: [
      {
        queue: containersQueue,
        settings: {
          maxWaitTimeMs: 250, // default is 500, maybe experiment with this
          maxConcurrency: 1,
          // maxConcurrency
          // deadLetterQueue: "failed-tasks", // Send failed messages to DLQ
        },
      },
    ],
  },
);

const browserContainer = await Container<BrowserContainer>(
  "browser-container",
  {
    className: "BrowserContainer",
    adopt: true,
    instanceType: "standard-4",
    // instanceType: BROWSER_CONTAINER_INSTANCE_TYPE,
    // schedulingPolicy: "moon",
    // maxInstances: 100000000000,
    build: {
      context: import.meta.dirname,
      dockerfile: "Dockerfile",
    },
  },
);

const browserContainerSidecar = DurableObjectNamespace<BrowserContainerSidecar>(
  "browser-container-sidecar-do",
  {
    className: "BrowserContainerSidecar",
    sqlite: true,
  },
);

export const server = await Worker("server", {
  entrypoint: "src/index.ts",
  compatibility: "node",
  bindings: {
    DB: db,
    KV: kv,
    CONTAINERS_QUEUE: containersQueue,
    BROWSER_CONTAINER: browserContainer,
    BROWSER_CONTAINER_SIDECAR_DO: browserContainerSidecar,
    // CONTAINER_ROUTER_DO: browserRouter,
    CORS_ORIGIN: alchemy.env.CORS_ORIGIN as Binding,
  },
  dev: {
    port: 3000,
  },
});

console.log(`Server -> ${server.url}`);

if (process.env.ALCHEMY_E2E) {
  await new Promise((resolve) => setTimeout(resolve, 1000));
  const { test } = await import("./test/e2e.js");
  try {
    await test(server.url);
  } catch (e) {
    console.error(e);
    exit(1);
  }
}

await app.finalize();
