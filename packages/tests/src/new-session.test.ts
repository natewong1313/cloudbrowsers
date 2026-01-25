import puppeteer from "puppeteer-core";
const SERVER_URL = "http://localhost:7000";

await fetch(`${SERVER_URL}/test`);
// ...
await Bun.sleep(5000);

const response = await fetch(`${SERVER_URL}/sessions/new`, { method: "post" });
const body = await response.json();
const wsAddr = body["ws_addr"];

console.log("connecting to", wsAddr);
const browser = await puppeteer.connect({
  browserWSEndpoint: wsAddr,
});

console.log("getting pages");
const [page] = await browser.pages();
if (!page) throw new Error("no page");

await page.goto("https://developer.chrome.com/");

const fullTitle = await page.title();
console.log('The title of this blog post is "%s".', fullTitle);

await browser.close();
