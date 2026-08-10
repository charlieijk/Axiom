# Axiom Site

Dedicated public product website for Axiom, released under the shared **Beta 1.0** codename.

## Product boundary

- `app/` is the focused, read-only product presentation.
- The whole site is a static marketing page with **no live backend connection**:
  the project record (repository link, snapshot, launch path) is descriptive
  metadata about the main product, not a runtime integration.
- Simulation authority, credentials, private connectors, persistence, and production mutations stay in the main product.
- `.openai/hosting.json` contains the Sites hosting metadata copied into production builds.

## Development

Requires Node.js `>=22.13.0`.

```bash
npm ci
npm run lint
npm test
npm run start
```

`npm test` creates a production Vinext build and verifies its rendered product surface, responsive behavior, local assets, release identity, and public-only boundary.

## Release identity

- Display codename: `Beta 1.0`
- Package version: `1.0.0-beta.1`
- Site role: public product surface
- Source of truth: canonical Axiom main-product repository
