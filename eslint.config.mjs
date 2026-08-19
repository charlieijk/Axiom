// Flat ESLint config for the browser replay clients in web/.
//
// The two surfaces there are deliberately different module systems and the
// config has to say so, because getting it wrong makes the lint either useless
// or wrong:
//
//   * web/app.js is a CLASSIC script (`<script src>`), so its top-level
//     `function` declarations are globals -- that is exactly how
//     web/tests/app.test.mjs reaches fetchReplay/tick through a vm context.
//     Parsed as a module it would gain module scope, and every one of those
//     declarations would look unused.
//   * web/graphics3d*.js are ES modules loaded with `<script type="module">`.
//
// site/ is a separate React/Vinext package with its own eslint config and its
// own CI job; it is ignored here so the two never fight over the same files.
import js from "@eslint/js";
import globals from "globals";

// Correctness-leaning rules on top of `js.configs.recommended`. Every one of
// these is already clean across web/, so they are ratchets against future
// regressions rather than a licence to reformat working code.
//
// `require-atomic-updates` was evaluated and deliberately left out: it flags
// five writes in web/app.js `fetchReplay` (`controls.play.textContent` and
// friends) because they happen after an `await`. `controls` is a `const` bound
// once at load and never reassigned, so there is nothing to read staler; the
// real staleness hazard -- a superseded replay landing late -- is already
// handled by the explicit `state.replayRequestId` guard and covered by tests.
const correctnessRules = {
  eqeqeq: ["error", "always"],
  "no-var": "error",
  "prefer-const": "error",
  "no-shadow": "error",
  "no-throw-literal": "error",
  "no-implicit-coercion": "error",
  "consistent-return": "error",
};

export default [
  {
    ignores: [
      "node_modules/**",
      "target/**",
      "site/**",
      // Vendored third-party builds are checked in verbatim; linting them would
      // report on code we deliberately do not modify.
      "web/vendor/**",
    ],
  },
  js.configs.recommended,
  {
    // The classic script. Browser globals only, script scope.
    files: ["web/app.js"],
    languageOptions: {
      ecmaVersion: 2023,
      sourceType: "script",
      globals: globals.browser,
    },
    rules: correctnessRules,
  },
  {
    // The 3D field journal's ES modules.
    files: ["web/graphics3d*.js"],
    languageOptions: {
      ecmaVersion: 2023,
      sourceType: "module",
      globals: globals.browser,
    },
    rules: correctnessRules,
  },
  {
    // The node:test suites and their DOM stubs.
    files: ["web/tests/**/*.mjs"],
    languageOptions: {
      ecmaVersion: 2023,
      sourceType: "module",
      globals: { ...globals.node, ...globals.browser },
    },
    rules: correctnessRules,
  },
];
