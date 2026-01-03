import { env } from "cloudflare:workers";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger } from "hono/logger";
export { ContainerRouter } from "./container-router";
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

app.get("/connect", (c) => {
  console.log("get do instance");
  const stub = env.CONTAINER_ROUTER_DO.getByName("test");
  console.log("forward to do instance");
  return stub.fetch(c.req.raw);
});

export default app;
