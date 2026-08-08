import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

async function render() {
  const workerUrl = new URL("../dist/server/index.js", import.meta.url);
  workerUrl.searchParams.set("test", `${process.pid}-${Date.now()}-${Math.random()}`);
  const { default: worker } = await import(workerUrl.href);

  return worker.fetch(
    new Request("http://localhost/", {
      headers: { accept: "text/html" },
    }),
    {
      ASSETS: {
        fetch: async () => new Response("Not found", { status: 404 }),
      },
    },
    {
      waitUntil() {},
      passThroughOnException() {},
    },
  );
}

test("server-renders the dedicated product site", async () => {
  const response = await render();
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /^text\/html\b/i);

  const html = await response.text();
  assert.match(html, /<title>[^<]+<\/title>/i);
  assert.match(html, /<meta(?=[^>]*\bname=["']description["'])(?=[^>]*\bcontent=["'][^"']+["'])[^>]*>/i);
  assert.match(html, /<main\b/i);
  assert.match(html, /<h1\b[^>]*>.+?<\/h1>/is);
  assert.doesNotMatch(html, /codex-preview|Your site is taking shape|Building your site|react-loading-skeleton/i);
  assert.doesNotMatch(html, /OPENAI_API_KEY|sk-[A-Za-z0-9_-]{20,}|tunnel_[a-f0-9]{32}/i);
  assert.doesNotMatch(html, /<script[^>]+src=["']https?:\/\//i);

  if (/role=["']tablist["']/.test(html)) {
    assert.match(html, /role=["']tab["']/);
    assert.match(html, /aria-selected=["']true["']/);
    assert.match(html, /aria-selected=["']false["']/);
  }
});

test("keeps the production site responsive and free of preview scaffolding", async () => {
  const [page, layout, css, packageJson] = await Promise.all([
    readFile(new URL("../app/page.tsx", import.meta.url), "utf8"),
    readFile(new URL("../app/layout.tsx", import.meta.url), "utf8"),
    readFile(new URL("../app/globals.css", import.meta.url), "utf8"),
    readFile(new URL("../package.json", import.meta.url), "utf8"),
  ]);

  assert.doesNotMatch(page, /codex-preview|_sites-preview|SkeletonPreview/);
  assert.doesNotMatch(page, /^\s*\+<section/m);
  assert.doesNotMatch(layout, /codex-preview|_sites-preview|SkeletonPreview/);
  assert.doesNotMatch(packageJson, /react-loading-skeleton/);
  assert.match(css, /@media\s*\([^)]*max-width/i);
  assert.match(css, /prefers-reduced-motion/i);
});
