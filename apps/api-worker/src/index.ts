import { env } from "cloudflare:workers";
import { Hono } from "hono";
import { logger } from "hono/logger";

const app = new Hono();
app.use(logger());

app.post("/sessions/new", async (c) => {
  const id = env.BROWSER_CONTAINER_DO.newUniqueId();
  const stub = env.BROWSER_CONTAINER_DO.get(id);
  await stub.init();
  await stub.newSession();

  return c.json({ hello: "world" });
});

export default app;
export { BrowserContainerDurableObject } from "./bindings/browser-container-do";
export { BrowserContainer } from "./bindings/browser-container";
