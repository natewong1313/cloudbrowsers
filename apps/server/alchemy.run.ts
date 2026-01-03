import type { BrowserContainer } from "@/browser-container/container";
import { ContainerRouter } from "./src/container-router";
import alchemy from "alchemy";
import {
  Container,
  DurableObjectNamespace,
  Worker,
  type Binding,
} from "alchemy/cloudflare";
import { D1Database } from "alchemy/cloudflare";
import { config } from "dotenv";

config({ path: "./.env" });

const app = await alchemy("browser-shop");

const db = await D1Database("database", {
  migrationsDir: "packages/db/src/migrations",
});

const browserContainer = await Container<BrowserContainer>(
  "browser-container",
  {
    className: "BrowserContainer",
    adopt: true,
    build: {
      context: import.meta.dirname,
      dockerfile: "Dockerfile",
    },
  },
);

export const server = await Worker("server", {
  entrypoint: "src/index.ts",
  compatibility: "node",
  bindings: {
    DB: db,
    BROWSER_CONTAINER: browserContainer,
    CONTAINER_ROUTER_DO: DurableObjectNamespace<ContainerRouter>(
      "container-router",
      {
        className: "ContainerRouter",
        sqlite: true,
      },
    ),
    CORS_ORIGIN: alchemy.env.CORS_ORIGIN as Binding,
  },
  dev: {
    port: 3000,
  },
});

console.log(`Server -> ${server.url}`);

await app.finalize();
