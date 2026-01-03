import { DurableObject, env } from "cloudflare:workers";
import {
  Container,
  getContainer,
  getRandom,
  switchPort,
} from "@cloudflare/containers";

export class ContainerRouter extends DurableObject {
  async fetch(req: Request): Promise<Response> {
    if (req.headers.get("upgrade") !== "websocket") {
      return new Response("Not a websocket request", { status: 400 });
    }

    console.log("get container");
    const container = getContainer(env.BROWSER_CONTAINER, "testcontainer");
    console.log("start");
    // await container.startAndWaitForPorts();
    return await container.fetch(switchPort(req, 6700));

    const wsPair = new WebSocketPair();
    const { 0: client, 1: server } = wsPair;
    this.ctx.acceptWebSocket(server);
    console.log("accepting websocket request");
    return new Response(null, {
      status: 101,
      webSocket: client,
    });
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
