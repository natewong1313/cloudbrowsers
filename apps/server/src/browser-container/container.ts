import { Container } from "@cloudflare/containers";

export class BrowserContainer extends Container {
  // Port the container listens on (default: 8080)
  defaultPort = 6700;
  // Time before container sleeps due to inactivity (default: 30s)
  sleepAfter = "2m";
  // Environment variables passed to the container
  // envVars = {
  //   MESSAGE: "I was passed in via the container class!",
  // };
}
