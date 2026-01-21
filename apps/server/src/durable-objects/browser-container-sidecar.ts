import { getContainer } from "@cloudflare/containers";
import { DurableObject, env } from "cloudflare:workers";

type BrowserContainerId = string & { __brand: "BrowserContainerId" };

// Sidecar DO to the BrowserContainer
// By creating this DO in x location we should be able to provision a container close by
export class BrowserContainerSidecar extends DurableObject {
  async init(containerId: BrowserContainerId) {
    const container = getContainer(env.BROWSER_CONTAINER, containerId);
    console.log("Starting container");
    await container.startAndWaitForPorts();
    console.log("Started container");
  }
}
