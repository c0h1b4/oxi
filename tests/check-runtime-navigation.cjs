/* Exercise the production export and real API with a minimal local IMAP fixture.
 * PLAYWRIGHT_MODULE: installed Playwright module path (default: playwright).
 * CHROMIUM_EXECUTABLE: optional system Chromium path.
 * Usage: node tests/check-runtime-navigation.cjs <server binary> <frontend export>
 */
const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const { mkdtemp, rm } = require("node:fs/promises");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const readline = require("node:readline");
const { once } = require("node:events");
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright");

const binary = path.resolve(process.argv[2]);
const exported = path.resolve(process.argv[3]);

async function main() {
  // LOGIN/LOGOUT authenticate against a real TCP peer; LIST returns an empty mailbox.
  const connections = new Set();
  const imap = net.createServer(socket => {
    connections.add(socket);
    socket.on("close", () => connections.delete(socket));
    socket.write("* OK test IMAP ready\r\n");
    let pending = "";
    socket.on("error", () => {});
    socket.on("data", data => {
      pending += data.toString();
      while (pending.includes("\r\n")) {
        const end = pending.indexOf("\r\n");
        const line = pending.slice(0, end);
        pending = pending.slice(end + 2);
        const [tag, command] = line.split(" ");
        if (command?.toUpperCase() === "LOGOUT") {
          socket.end(`* BYE logging out\r\n${tag} OK LOGOUT completed\r\n`);
        } else if (command?.toUpperCase() === "SELECT") {
          socket.write(`* 0 EXISTS\r\n* OK [UIDVALIDITY 1] UIDs valid\r\n${tag} OK SELECT completed\r\n`);
        } else if (command?.toUpperCase() === "SEARCH") {
          socket.write(`* SEARCH\r\n${tag} OK SEARCH completed\r\n`);
        } else if (command?.toUpperCase() === "CAPABILITY") {
          socket.write(`* CAPABILITY IMAP4rev1\r\n${tag} OK CAPABILITY completed\r\n`);
        } else {
          socket.write(`${tag} OK ${command} completed\r\n`);
        }
      }
    });
  });
  imap.listen(0, "127.0.0.1");
  await once(imap, "listening");
  const browser = await chromium.launch({
    headless: true,
    ...(process.env.CHROMIUM_EXECUTABLE ? { executablePath: process.env.CHROMIUM_EXECUTABLE } : {}),
    args: ["--no-sandbox"],
  });
  try {
    for (const configured of ["", "/oxi", "/mail", "/apps/mail/"]) {
      const prefix = configured.replace(/\/+$/, "");
      const data = await mkdtemp(path.join(os.tmpdir(), "oxi-navigation-"));
      const portSocket = net.createServer();
      portSocket.listen(0, "127.0.0.1");
      await once(portSocket, "listening");
      const port = portSocket.address().port;
      await new Promise(resolve => portSocket.close(resolve));
      const context = await browser.newContext();
      const server = spawn(binary, [], { env: {
        ...process.env, HOST: "127.0.0.1", PORT: String(port), STATIC_DIR: exported,
        DATA_DIR: data, BASE_PATH: configured, ENVIRONMENT: "development",
        IMAP_HOST: "127.0.0.1", IMAP_PORT: String(imap.address().port),
        SMTP_HOST: "127.0.0.1", TLS_ENABLED: "false",
      }, stdio: ["ignore", "pipe", "inherit"] });
      // Subscribe before startup/navigation: the child may exit before cleanup runs.
      const exited = once(server, "exit");
      try {
        let ready = false;
        for await (const line of readline.createInterface({ input: server.stdout })) {
          if (line.includes("oxi-email server starting")) { ready = true; break; }
        }
        assert(ready, "server failed before startup");
        server.stdout.resume();
        const page = await context.newPage();
        const origin = `http://127.0.0.1:${port}`;
        const errors = [];
        const rootLeaks = [];
        const badResponses = [];
        const documents = [];
        page.on("pageerror", error => errors.push(error.message));
        page.on("request", request => {
          const url = new URL(request.url());
          if (request.resourceType() === "document") documents.push(url.pathname);
          if (url.origin === origin && !url.pathname.startsWith(prefix + "/")) rootLeaks.push(url.pathname);
        });
        page.on("response", response => {
          if (response.status() >= 400) badResponses.push(`${response.status()} ${response.url()}`);
        });
        // Direct app load must navigate client-side to login with the prefix intact.
        await page.goto(origin + prefix + "/");
        await page.locator("#email").waitFor();
        assert.equal(new URL(page.url()).pathname, prefix + "/login/");
        // Direct login load must also hydrate and find its own assets.
        await page.reload();
        await page.locator("#email").fill("alice@example.com");
        await page.locator("#password").fill("test-password");
        await page.getByRole("button", { name: /Server configuration/ }).click();
        // Configure the fixture rather than contacting a real mail server.
        await page.locator("#imapHost").fill("127.0.0.1");
        await page.locator("#imapPort").fill(String(imap.address().port));
        await page.locator("#imapTls").uncheck();
        await page.locator("#smtpTls").uncheck();
        const login = page.waitForResponse(r => r.url().endsWith(prefix + "/api/auth/login"));
        await page.getByRole("button", { name: "Sign in", exact: true }).click();
        const loginResponse = await login;
        assert.equal(loginResponse.status(), 201, await loginResponse.text());
        await page.getByRole("navigation", { name: "Main navigation" }).waitFor();
        assert.equal(new URL(page.url()).pathname, prefix + "/");
        const cookies = (await context.cookies()).filter(c => c.name.startsWith("oxi_"));
        assert(cookies.length >= 2);
        for (const cookie of cookies) assert.equal(cookie.path, prefix || "/");
        // Explicitly verify an authenticated WebSocket upgrade, not just its URL.
        await page.evaluate(url => new Promise((resolve, reject) => {
          const socket = new WebSocket(url);
          socket.onopen = () => { socket.close(); resolve(true); };
          socket.onerror = () => reject(new Error("WebSocket upgrade failed"));
        }), origin.replace("http:", "ws:") + prefix + "/api/ws");
        const logout = page.waitForResponse(r => r.url().endsWith(prefix + "/api/auth/logout"));
        await page.getByRole("button", { name: "Logout", exact: true }).click();
        assert.equal((await logout).status(), 200);
        await page.locator("#email").waitFor();
        assert.equal(new URL(page.url()).pathname, prefix + "/login/");
        assert.equal(documents.length, 2, "login/logout should use client-side navigation");
        assert.deepEqual(rootLeaks, []);
        assert.deepEqual(errors, []);
        assert.deepEqual(badResponses, []);
        console.log(`Browser prefix ${configured || "(root)"}: hydration, login/logout, cookies, navigation, assets, and WebSocket OK`);
      } finally {
        await context.close();
        server.kill();
        await exited;
        await rm(data, { recursive: true, force: true });
      }
    }
  } finally {
    await browser.close();
    for (const socket of connections) socket.destroy();
    imap.close();
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
