import alchemy from "alchemy";
import { Worker } from "alchemy/cloudflare";
import { D1Database } from "alchemy/cloudflare";
import { config } from "dotenv";

config({ path: "./.env" });

const app = await alchemy("browser-shop");

const db = await D1Database("database", {
	migrationsDir: "packages/db/src/migrations",
});

export const server = await Worker("server", {
	entrypoint: "src/index.ts",
	compatibility: "node",
	bindings: {
		DB: db,
		CORS_ORIGIN: alchemy.env.CORS_ORIGIN,
	},
	dev: {
		port: 3000,
	},
});

console.log(`Server -> ${server.url}`);

await app.finalize();
