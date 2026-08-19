// Invariants about the shipped pages themselves rather than their behaviour.
//
// A JS linter cannot see any of this: there is no JSX for jsx-a11y to read and
// no React for react-hooks to check, so the accessibility and supply-chain
// rules that would live in an eslint plugin in a component app are asserted
// here, against the real HTML and CSS the server sends.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = (name) => readFile(new URL(`../${name}`, import.meta.url), "utf8");

const pages = {
  "index.html": await read("index.html"),
  "graphics3d.html": await read("graphics3d.html"),
};
const sheets = {
  "styles.css": await read("styles.css"),
  "graphics3d.css": await read("graphics3d.css"),
};
const phosphorCss = await read("vendor/phosphor-icons.css");
const fontsCss = await read("vendor/fonts.css");

/** Every src/href the page asks the browser to fetch. */
function references(html) {
  return [...html.matchAll(/\b(?:src|href)="([^"]+)"/g)].map((match) => match[1]);
}

test("no page fetches anything from a host Axiom does not control", () => {
  for (const [name, html] of Object.entries(pages)) {
    const external = references(html).filter((url) => /^(https?:)?\/\//.test(url));
    assert.deepEqual(
      external,
      [],
      `${name} must vendor its assets; a CDN reference is an unpinned dependency ` +
        `on a third party (and leaks every visitor's IP to them)`,
    );
    assert.doesNotMatch(html, /rel="(pre(connect|load|fetch)|dns-prefetch)"[^>]*https?:/, name);
  }
});

test("local asset references carry no hand-maintained cache-busting token", () => {
  // src/web.rs answers every request with `Cache-Control: no-store`, so a `?v=N`
  // busts nothing; all it can do is drift from the file it claims to version.
  for (const [name, html] of Object.entries(pages)) {
    const versioned = references(html).filter((url) => url.includes("?v="));
    assert.deepEqual(versioned, [], `${name} still hand-versions an asset URL`);
  }
});

test("every icon the 3D page uses is defined by the vendored stylesheet", () => {
  const used = new Set(
    [...pages["graphics3d.html"].matchAll(/class="ph (ph-[a-z0-9-]+)"/g)].map((match) => match[1]),
  );
  assert.ok(used.size >= 10, `expected the page to use icons, found ${used.size}`);
  for (const icon of used) {
    assert.match(
      phosphorCss,
      new RegExp(`\\.${icon}:before`),
      `${icon} is used in graphics3d.html but not defined in web/vendor/phosphor-icons.css`,
    );
  }
});

test("the vendored stylesheets carry their fonts inline rather than by url", () => {
  for (const [name, css] of Object.entries({ "phosphor-icons.css": phosphorCss, "fonts.css": fontsCss })) {
    assert.match(css, /@font-face/, `${name} must declare its face`);
    assert.match(css, /src:\s*url\((?:"|')?data:font\/woff2;base64,/, `${name} must inline its font`);
    const relative = [...css.matchAll(/url\((?!["']?data:)([^)]*)\)/g)].map((match) => match[1]);
    assert.deepEqual(relative, [], `${name} still points at a font file the server does not serve`);
  }
});

test("the vendored font stacks keep a system fallback", () => {
  // If the webfont fails, the page must still set type rather than fall all the
  // way back to the browser default.
  const stacks = [...sheets["graphics3d.css"].matchAll(/font-family: ([^;]+);/g)].map((m) => m[1]);
  assert.ok(stacks.length > 0);
  for (const stack of stacks) {
    const families = stack.split(",").map((part) => part.trim());
    assert.ok(
      families.length > 1,
      `"${stack}" names a single family; a webfont failure would leave it unstyled`,
    );
  }
});

test("both pages declare a language, a charset and a responsive viewport", () => {
  for (const [name, html] of Object.entries(pages)) {
    assert.match(html, /<html lang="en">/, `${name} must declare its language for screen readers`);
    assert.match(html, /<meta charset="utf-8" \/>/, name);
    assert.match(
      html,
      /<meta name="viewport" content="width=device-width, initial-scale=1" \/>/,
      `${name} must scale to the device, not to a fixed desktop width`,
    );
  }
});

test("every button has an accessible name", () => {
  for (const [name, html] of Object.entries(pages)) {
    const buttons = [...html.matchAll(/<button\b([^>]*)>([\s\S]*?)<\/button>/g)];
    assert.ok(buttons.length > 0, `${name} should have buttons`);
    for (const [, attributes, body] of buttons) {
      // Text the assistive layer can read: an aria-label, or visible text that
      // is not just an aria-hidden icon.
      const visibleText = body
        .replace(/<i\b[^>]*><\/i>/g, "")
        .replace(/<[^>]+>/g, "")
        .trim();
      assert.ok(
        /aria-label="/.test(attributes) || visibleText.length > 0,
        `${name} has a button with no accessible name: <button${attributes}>${body.trim()}`,
      );
    }
  }
});

const VOID_ELEMENTS = new Set(["area", "base", "br", "col", "hr", "img", "input", "link", "meta", "source"]);

/**
 * Every `<i>` in the document, paired with whether anything above it is already
 * `aria-hidden`. A tiny stack scanner rather than a regex, because "the legend
 * wrapper is hidden, so its swatches are too" is the whole question here.
 */
function iconsWithHiddenAncestry(html) {
  const found = [];
  const stack = [];
  for (const match of html.matchAll(/<(\/?)([a-z0-9]+)\b([^>]*)>/g)) {
    const [, closing, tag, attributes] = match;
    const hiddenAbove = stack.some((frame) => frame.hidden);
    if (closing) {
      while (stack.length > 0 && stack.pop().tag !== tag) {
        // Tolerate an unclosed tag rather than derailing on it.
      }
      continue;
    }
    if (tag === "i") {
      found.push({ attributes, hiddenAbove });
    }
    if (!VOID_ELEMENTS.has(tag) && !attributes.trimEnd().endsWith("/")) {
      stack.push({ tag, hidden: /aria-hidden="true"/.test(attributes) });
    }
  }
  return found;
}

test("decorative icons are hidden from assistive technology", () => {
  // Only the 3D page uses an icon font; index.html is text-labelled throughout.
  assert.ok(iconsWithHiddenAncestry(pages["graphics3d.html"]).length >= 10);
  for (const [name, html] of Object.entries(pages)) {
    const icons = iconsWithHiddenAncestry(html);
    for (const { attributes, hiddenAbove } of icons) {
      assert.ok(
        hiddenAbove || /aria-hidden="true"/.test(attributes),
        `${name} has an icon a screen reader would announce as noise: <i${attributes}>`,
      );
    }
  }
});

test("every canvas and form control is labelled", () => {
  for (const [name, html] of Object.entries(pages)) {
    for (const [, attributes] of html.matchAll(/<canvas\b([^>]*)>/g)) {
      assert.match(attributes, /aria-label="/, `${name} has an unlabelled canvas: <canvas${attributes}>`);
    }

    const labelled = [...html.matchAll(/<label\b[^>]*>([\s\S]*?)<\/label>/g)].map((match) => match[1]);
    for (const [, tag, attributes] of html.matchAll(/<(select|input)\b([^>]*)>/g)) {
      const id = /id="([^"]+)"/.exec(attributes)?.[1];
      const inLabel = labelled.some((body) => body.includes(`id="${id}"`));
      assert.ok(
        inLabel || /aria-label="/.test(attributes),
        `${name}: <${tag} id="${id}"> is neither wrapped in a <label> nor aria-labelled`,
      );
    }
  }
});

test("async views announce themselves as live regions", () => {
  // The evolution run can take seconds; a sighted user sees the veil, and these
  // are what a screen-reader user gets instead.
  const html = pages["graphics3d.html"];
  assert.match(html, /id="loading-state"[^>]*role="status"[^>]*aria-live="polite"/);
  assert.match(html, /id="archive-detail"[^>]*aria-live="polite"/);
  assert.match(html, /class="selected-generation" aria-live="polite"/);
});

test("both stylesheets honour prefers-reduced-motion", () => {
  for (const [name, css] of Object.entries(sheets)) {
    assert.match(
      css,
      /@media \(prefers-reduced-motion: reduce\)/,
      `${name} must have a reduced-motion escape hatch`,
    );
  }
});

test("both stylesheets commit to their single colour scheme explicitly", () => {
  // Axiom's two surfaces are deliberately dark-only -- see the note in each
  // sheet. `color-scheme` is what tells the browser to match its own form
  // controls and scrollbars to that, instead of painting light chrome on a dark
  // page.
  for (const [name, css] of Object.entries(sheets)) {
    assert.match(css, /color-scheme: dark;/, `${name} must declare its scheme`);
    assert.match(
      css,
      /prefers-color-scheme/,
      `${name} must say in the sheet why it does not follow the system scheme`,
    );
  }
});

test("both stylesheets lay out at a 375px phone", () => {
  // 375px is the iPhone SE/12 mini class of device and the narrowest width the
  // UI checklist requires. A breakpoint that stops at 520/620px leaves that
  // width to whatever the desktop rules happen to do.
  for (const [name, css] of Object.entries(sheets)) {
    const widths = [...css.matchAll(/@media \(max-width: (\d+)px\)/g)].map((match) =>
      Number(match[1]),
    );
    assert.ok(
      widths.some((width) => width <= 400),
      `${name} has no breakpoint at or below 400px; narrowest is ${Math.min(...widths)}px`,
    );
  }
});
