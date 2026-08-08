import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
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
  assert.match(html, /<link(?=[^>]*\brel=["']icon["'])(?=[^>]*\bhref=["']\/(?:icon\.svg\?[^"']+|favicon\.svg)["'])[^>]*>/i);
  assert.match(html, /<main\b/i);
  assert.match(html, /<h1\b[^>]*>.+?<\/h1>/is);
  assert.doesNotMatch(html, /codex-preview|Your site is taking shape|Building your site|react-loading-skeleton/i);
  assert.doesNotMatch(html, /OPENAI_API_KEY|sk-[A-Za-z0-9_-]{20,}|tunnel_[a-f0-9]{32}/i);
  assert.doesNotMatch(html, /<script[^>]+src=["']https?:\/\//i);
  assert.match(html, /Beta 1\.0/i);

  if (/role=["']tablist["']/.test(html)) {
    assert.match(html, /role=["']tab["']/);
    assert.match(html, /aria-selected=["']true["']/);
    assert.match(html, /aria-selected=["']false["']/);
  }
});

test("keeps the production site responsive and free of preview scaffolding", async () => {
  const [page, layout, css, packageJson, icon] = await Promise.all([
    readFile(new URL("../app/page.tsx", import.meta.url), "utf8"),
    readFile(new URL("../app/layout.tsx", import.meta.url), "utf8"),
    readFile(new URL("../app/globals.css", import.meta.url), "utf8"),
    readFile(new URL("../package.json", import.meta.url), "utf8"),
    readFile(new URL("../app/icon.svg", import.meta.url), "utf8"),
  ]);

  assert.doesNotMatch(page, /codex-preview|_sites-preview|SkeletonPreview/);
  assert.doesNotMatch(page, /^\s*\+<section/m);
  assert.doesNotMatch(layout, /codex-preview|_sites-preview|SkeletonPreview/);
  assert.doesNotMatch(packageJson, /react-loading-skeleton/);
  assert.match(packageJson, /"version"\s*:\s*"1\.0\.0-beta\.1"/);
  assert.match(css, /@media\s*\([^)]*max-width/i);
  assert.match(css, /prefers-reduced-motion/i);
  assert.match(icon, /^<svg[^>]+viewBox="0 0 64 64"/);
  assert.doesNotMatch(icon, /\b(?:href|src)=["']https?:\/\//i);

  const proofAsset = page.match(/src="\/(original-interface\.(png|jpe?g))"/i);
  if (proofAsset) {
    const bytes = await readFile(
      new URL(`../public/${proofAsset[1]}`, import.meta.url),
    );
    assert.ok(bytes.length <= 2_500_000, `${proofAsset[1]} exceeds the 2.5 MB proof-image budget`);
    if (proofAsset[2].toLowerCase() === "png") {
      assert.deepEqual([...bytes.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
    } else {
      assert.deepEqual([...bytes.subarray(0, 3)], [255, 216, 255]);
    }
    assert.match(page, /<img[^>]+alt="[^"]+"[^>]+loading="lazy"/i);
  }
});

test("contains only the dedicated Site product surface", async () => {
  const [readme, packageJson, viteConfig, packagingPlugin, worker] = await Promise.all([
    readFile(new URL("../README.md", import.meta.url), "utf8"),
    readFile(new URL("../package.json", import.meta.url), "utf8"),
    readFile(new URL("../vite.config.ts", import.meta.url), "utf8"),
    readFile(new URL("../build/sites-vite-plugin.ts", import.meta.url), "utf8"),
    readFile(new URL("../worker/index.ts", import.meta.url), "utf8"),
  ]);

  assert.doesNotMatch(readme, /vinext-starter|starter template|optional D1|db:generate/i);
  assert.doesNotMatch(packageJson, /drizzle-orm|drizzle-kit|db:generate/i);
  assert.doesNotMatch(viteConfig, /D1Database|d1_databases|r2_buckets|SITE_CREATOR_PLACEHOLDER_DATABASE_ID/);
  assert.doesNotMatch(packagingPlugin, /drizzle|migrations/i);
  assert.doesNotMatch(worker, /D1Database|vinext-starter/i);

  for (const relativePath of [
    "../app/chatgpt-auth.ts",
    "../db/index.ts",
    "../db/schema.ts",
    "../drizzle.config.ts",
    "../drizzle/meta/_journal.json",
    "../examples/d1/app/api/notes/route.ts",
    "../examples/d1/db/schema.ts",
    "../public/file.svg",
    "../public/globe.svg",
    "../public/window.svg",
  ]) {
    await assert.rejects(access(new URL(relativePath, import.meta.url)), {
      code: "ENOENT",
    });
  }
});
