import { switchPort } from "@cloudflare/containers";
import { env } from "cloudflare:workers";
import { Hono } from "hono";
import { logger } from "hono/logger";
import { BROWSER_CONTAINER_WS_PORT } from "./bindings/browser-container";
import { COLO_TO_REGION_MAP, type Colo } from "./utils/regions";

const app = new Hono();
app.use(logger());

app.get("/test", async (c) => {
  const reqColo = c.req.raw.cf?.colo as Colo;
  const region = COLO_TO_REGION_MAP[reqColo];
  if (!region) {
    throw new Error("unexpected error getting region");
  }

  const router = env.BROWSER_CONTAINER_ROUTER.getByName(region);
  // always call this
  await router.init(region);
  const sessionResult = await router.requestSession();

  return c.json({
    sessionId: sessionResult.sessionId,
    wsConnectUrl: `ws://localhost:7000${sessionResult.wsConnectPath}`,
  });
});

app.get("/session/:containerId/:sessionId", async (c) => {
  const { containerId, sessionId } = c.req.param();

  const container = env.BROWSER_CONTAINER.getByName(containerId);
  // Proxy over the ws connection
  const req = switchPort(
    new Request(`http://container/session/${sessionId}`, c.req.raw),
    BROWSER_CONTAINER_WS_PORT,
  );
  return await container.fetch(req);
});

export default app;
export { BrowserContainer } from "./bindings/browser-container";
export { BrowserContainerRouter } from "./bindings/browser-container-router";
