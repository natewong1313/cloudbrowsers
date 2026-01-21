import puppeteer from "puppeteer-core";

const sleep = (ms: number) => {
  return new Promise((resolve) => setTimeout(resolve, ms));
};

const browser = await puppeteer.connect({
  browserWSEndpoint: "ws://127.0.0.1:3000/connect",
  // "ws://127.0.0.1:59641/devtools/browser/0fb40df1-6c14-40f5-bb45-9b0837b092d5",
});

console.log("getting pages");
const [page] = await browser.pages();
if (!page) throw new Error("no page");

console.log("got page");
// Navigate the page to a URL.
await page.goto("https://developer.chrome.com/");

const fullTitle = await page.title();
console.log('The title of this blog post is "%s".', fullTitle);

await sleep(5000);

await browser.close();
