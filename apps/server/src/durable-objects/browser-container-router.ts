import { DurableObject, env } from "cloudflare:workers";
import { getContainer, switchPort } from "@cloudflare/containers";

export class BrowserContainerRouter extends DurableObject {
  async fetch(req: Request): Promise<Response> {
    if (req.headers.get("upgrade") !== "websocket") {
      return new Response("Not a websocket request", { status: 400 });
    }

    // Forward the websocket request to the underlying container
    const container = getContainer(env.BROWSER_CONTAINER, "testcontainer");
    return await container.fetch(switchPort(req, 6700));
  }

  webSocketMessage(
    ws: WebSocket,
    message: string | ArrayBuffer,
  ): void | Promise<void> {
    console.log("message", message);
    ws.send(`Received message: ${message.toString()}`);
  }

  webSocketClose(
    _ws: WebSocket,
    code: number,
    reason: string,
  ): void | Promise<void> {
    console.log("close", code, reason);
  }

  webSocketError(_ws: WebSocket, error: unknown): void | Promise<void> {
    console.log("error", error);
  }
}
