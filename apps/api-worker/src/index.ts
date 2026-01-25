import { switchPort } from "@cloudflare/containers";
import { env } from "cloudflare:workers";
import { Hono } from "hono";
import { logger } from "hono/logger";
import { BROWSER_CONTAINER_WS_PORT } from "./bindings/browser-container";

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
  const { sessionId, wsConnectPath } = await stub.newSession();

  return c.json({
    sessionId,
    wsConnectUrl: `ws://localhost:7000${wsConnectPath}`,
  });
});

app.get("/session/:containerId/:sessionId", async (c) => {
  const { containerId, sessionId } = c.req.param();

  // Bypass going through the container DO to avoid an extra network trip
  const container = env.BROWSER_CONTAINER.getByName(containerId);
  const req = switchPort(
    new Request(`http://container/session/${sessionId}`, c.req.raw),
    BROWSER_CONTAINER_WS_PORT,
  );
  return await container.fetch(req);
});

export default app;
export { BrowserContainerDurableObject } from "./bindings/browser-container-do";
export { BrowserContainer } from "./bindings/browser-container";
