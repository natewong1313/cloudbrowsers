import { env } from "cloudflare:workers";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger } from "hono/logger";
import type { BrowserContainerId } from "./browser-container/container";

export { BrowserContainerSidecar } from "./durable-objects/browser-container-sidecar";
export { BrowserContainer } from "./browser-container/container";

const app = new Hono();

app.use(logger());
app.use(
  "/*",
  cors({
    origin: env.CORS_ORIGIN || "",
    allowMethods: ["GET", "POST", "OPTIONS"],
    allowHeaders: ["Content-Type", "Authorization"],
    credentials: true,
  }),
);

const USER = "nate";

/**
 * dont like this api but it gets the job done
 */
app.post("/sessions/new", async (c) => {
  await env.CONTAINERS_QUEUE.send({ userId: USER });
  return c.json({ hello: "world" });
});

app.get("/connect", (c) => {
  const stub = env.BROWSER_CONTAINER_SIDECAR_DO.getByName("test", {
    locationHint: "enam",
  });
  console.log("forward to do instance");
  return stub.fetch(c.req.raw);
});

export default app;
