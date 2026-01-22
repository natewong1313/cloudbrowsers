import puppeteer from "puppeteer-core";
import assert from "node:assert";

export const test = async (serverUrl?: string) => {
  assert(serverUrl, "server url not provided");
  await pollUntilReady(serverUrl);

  const baseUrl = serverUrl.endsWith("/") ? serverUrl.slice(0, -1) : serverUrl;
  const wsEndpoint = baseUrl.replace("http", "ws") + "/connect";

  console.log("connecting to browser at", wsEndpoint);
  const browser = await puppeteer.connect({
    browserWSEndpoint: wsEndpoint,
  });

  console.log("Getting pages");
  const [page] = await browser.pages();
  assert(page, "No page available");

  console.log("navigating");
  await page.goto("https://developer.chrome.com/");

  const title = await page.title();
  console.log("title", title);

  await browser.close();

  console.log("yay");
};

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const pollUntilReady = async (url: string) => {
  let attempts = 0;
  while (attempts < 30) {
    try {
      const res = await fetch(url);
      if (res.ok || res.status < 500) {
        return;
      }
    } catch {}
    await sleep(1000);
    attempts++;
  }
  throw new Error(`poll timeout after 30 seconds: ${url}`);
};
