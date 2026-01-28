import { switchPort } from "@cloudflare/containers";
import { env } from "cloudflare:workers";
import { Hono } from "hono";
import { logger } from "hono/logger";
import {
  BROWSER_CONTAINER_WS_PORT,
  ContainerFetchError,
  newBrowserContainerId,
  NoCapacityError,
} from "./bindings/browser-container";

const app = new Hono();
app.use(logger());

app.get("/test", async (c) => {
  const id = newBrowserContainerId();

  const stub = env.BROWSER_CONTAINER.getByName(id);
  await stub.init(id);

  const sessionResult = await stub.newSession();
  if (sessionResult instanceof NoCapacityError) {
    throw new Error(sessionResult.message);
  } else if (sessionResult instanceof ContainerFetchError) {
    throw new Error(sessionResult.message);
  } else {
    return c.json({
      sessionId: sessionResult.sessionId,
      wsConnectUrl: `ws://localhost:7000${sessionResult.wsConnectPath}`,
    });
  }
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
