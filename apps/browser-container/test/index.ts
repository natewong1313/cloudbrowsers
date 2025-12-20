import puppeteer from "puppeteer-core";

const browser = await puppeteer.connect({
	browserWSEndpoint: "ws://127.0.0.1:6700/connect",
	// "ws://127.0.0.1:59641/devtools/browser/0fb40df1-6c14-40f5-bb45-9b0837b092d5",
});
// const browser = await puppeteer.launch({
// 	executablePath:
// 		"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
// });
const page = await browser.newPage();

// Navigate the page to a URL.
await page.goto("https://developer.chrome.com/");

const fullTitle = await page.title();
console.log('The title of this blog post is "%s".', fullTitle);

await browser.close();
