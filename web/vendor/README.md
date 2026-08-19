# Vendored third-party assets

`/3d` renders nothing over the network that Axiom does not ship itself. Every
file here is checked in so the page works offline, at judge time, and behind a
blocked CDN, and so no third-party host sees a visitor's IP.

## Three.js

`three.module.min.js` is the unmodified minified ES module from the official
`three@0.165.0` npm package.

- Package: `three@0.165.0`
- Registry tarball: `https://registry.npmjs.org/three/-/three-0.165.0.tgz`
- Tarball SHA-256: `fbe58b074968955ac2089b0cd70acd7dd9f3992e137760cb56541c2dbeae9952`
- Module SHA-256: `1af5bef9a9fd79fcb73fcfd3e4dd2ac0cea8720205bc7607d67369ce194a87fa`
- License: MIT; see `THREE-LICENSE.txt`

## Phosphor Icons

`phosphor-icons.css` replaces the `https://unpkg.com/@phosphor-icons/web@2.1.1`
`<script>` the page used to load with no SRI hash and no fallback. That script
injected six stylesheets — one per weight — from unpkg; `/3d` only ever uses the
regular weight (`class="ph ph-…"`), so only regular is vendored.

It is `src/regular/style.css` from the package, verbatim apart from the
`@font-face` `src` list: upstream points at four sibling font files, and the
vendored copy carries `Phosphor.woff2` inline as a `data:` URI instead. The icon
class list is untouched.

Inlining rather than shipping the `.woff2` beside it is deliberate. Axiom's
server (`src/web.rs`) builds every response from an `include_str!`ed `String`
and has no binary asset path at all, and it sends `Cache-Control: no-store` on
everything — so a separately cacheable font file would cost a route, a MIME
branch and a response-writer rework while buying nothing.

- Package: `@phosphor-icons/web@2.1.1`
- Registry tarball:
  `https://registry.npmjs.org/@phosphor-icons/web/-/web-2.1.1.tgz`
- Tarball SHA-256: `c74ab1decb33d7cefa22a7c412345e30d28ac39941b3be6837bc956cbef654d0`
- Tarball SHA-512 (matches the registry's published `dist.integrity`):
  `QjrfbItu5Rb2i37GzsKxmrRHfZPTVk3oXSPBnQ2+oACDbQRWGAeB0AsvZw263n1nFouQuff+khOCtRbrc6+k+A==`
- `src/regular/style.css` SHA-256:
  `873761b8711147dc516b6102936e9ad005f3a3015349efcde1a496f0326f1051`
- `src/regular/Phosphor.woff2` SHA-256 (the bytes behind the `data:` URI):
  `c2ea45ea05ff5c7df1936770c104725f2a68f43fd343f35f3da23a30b27de32a`
- License: MIT; see `PHOSPHOR-LICENSE.txt`

## Manrope and Newsreader

`fonts.css` replaces the two `https://cdn.jsdelivr.net/npm/@fontsource-variable/…`
stylesheets the page used to load, on the same reasoning as the icons.

Only the `latin` subset of each family is carried, and only the upright variable
face. `/3d` is `lang="en"` and every glyph it renders — including the `→`, `↑`
and `·` in the archive and journal copy — falls inside the latin
`unicode-range`; upstream's cyrillic, greek, vietnamese and latin-ext subsets
would be roughly 250KB that is never requested. No italic face is referenced by
`graphics3d.css`.

Each face is upstream's own `@font-face` block with the `src` url replaced by
the upstream `.woff2` inline as a `data:` URI. `font-weight`, `font-display` and
`unicode-range` are untouched, and both stacks in `graphics3d.css` still name a
system fallback after the webfont.

- Packages: `@fontsource-variable/manrope@5.1.1`,
  `@fontsource-variable/newsreader@5.1.1`
- Tarball SHA-512 (both match the registry's published `dist.integrity`):
  - manrope:
    `bJTED5u+nlztikeRJwCgK3ij3qMVsivP4xTxpcrfQ15crvZuW9hjM5f61CgYF3ryL0rJwkjzOpZjB4CwL/pnBw==`
  - newsreader:
    `cW9UGrGSPsPvrNV1uHrRo/21xNtVj/Im9b2mhComwopdXETjIbmqEY2T0akGwp1+FdSfQNpioiw+3FO8hemOhA==`
- `files/manrope-latin-wght-normal.woff2` SHA-256:
  `14be4114dcfde74652f19f9ffae8c9bb50707e9e88bd2b1fcd86fb50224109e7`
- `files/newsreader-latin-wght-normal.woff2` SHA-256:
  `1d90689c09f33ebf0b19f294047d9a21767bb3d505012eb75d88e303ac94c8ef`
- License: SIL Open Font License 1.1; see `MANROPE-LICENSE.txt` and
  `NEWSREADER-LICENSE.txt`

## Keeping this honest

`web/tests/assets.test.mjs` fails the build if any page reaches for an external
host, if `/3d` uses an icon class the vendored stylesheet does not define, or if
a vendored file loses its `@font-face`. `src/web.rs`'s smoke check
(`cargo run -- gui --check`) fails if a route stops serving one of these files.
