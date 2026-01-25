import { env } from "cloudflare:workers";
import { Hono } from "hono";
import { logger } from "hono/logger";

const app = new Hono();
app.use(logger());

const id = env.BROWSER_CONTAINER_DO.newUniqueId();

// await stub.newSession();
app.get("/test", async (c) => {
  const stub = env.BROWSER_CONTAINER_DO.get(id);
  await stub.init();
  return c.json({});
});

app.post("/sessions/new", async (c) => {
  const stub = env.BROWSER_CONTAINER_DO.get(id);
  const session = await stub.newSession();

  return c.json(session);
});

export default app;
export { BrowserContainerDurableObject } from "./bindings/browser-container-do";
export { BrowserContainer } from "./bindings/browser-container";
