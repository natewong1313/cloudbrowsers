import puppeteer from "puppeteer-core";
const SERVER_URL = "http://localhost:7000";

const response = await fetch(`${SERVER_URL}/test`);
const body = (await response.json()) as { wsConnectUrl: string };

// const response = await fetch(`${SERVER_URL}/sessions/new`, { method: "post" });
// const body = (await response.json()) as { wsConnectUrl: string };

console.log("connecting to", body);
const browser = await puppeteer.connect({
  browserWSEndpoint: body.wsConnectUrl,
});

console.log("getting pages");
const [page] = await browser.pages();
if (!page) throw new Error("no page");

await page.goto("https://developer.chrome.com/");

const fullTitle = await page.title();
console.log('The title of this blog post is "%s".', fullTitle);

await browser.close();
