# uncoil site

The website and documentation for uncoil: a landing page at `/` and Starlight docs at `/docs/`, built as one
static Astro project.

- Astro 7, Tailwind CSS 4 (preflight for the landing page), Starlight (docs). No client framework: the one
  script on the landing page is a few KB of plain TypeScript.
- Design: the app's Swatch Book (`DESIGN.md`). Tokens live once in `src/styles/tokens.css` (same names and
  values as `apps/uncoil/src/app.css`), imported by `src/styles/site.css` (landing page) and
  `src/styles/starlight.css` (docs). Light and dark follow the OS; the docs mirror it into `data-theme`
  (`src/components/docs/ThemeProvider.astro`), so there is no theme picker.
- Type is the system UI face (Segoe UI Variable on Windows). No web fonts, no third-party requests.

## Develop

```sh
cd site
pnpm install
pnpm dev        # http://localhost:4321/uncoil/
```

## Build and preview

```sh
pnpm build      # -> site/dist, fully static
pnpm preview    # serves dist at http://localhost:4321/uncoil/
```

The build must finish without warnings.

## What is generated, and from where

| On the site | Source |
|---|---|
| Lit desk (landing hero) | `src/components/DeskHero.astro`: the still frame is rendered at build time from `apps/uncoil/src/lib/mock/desk.json` and the app's own `apps/uncoil/src/lib/effect.ts` (through the `$uncoil` alias in `astro.config.mjs`); a small script animates it with the same `frame()` while on screen, and not under reduced motion unless the visitor presses Play |
| Fn layer with gel tabs (landing, "What it does") | `src/components/FnKeys.astro`: the keyboard's real layout from desk.json; the Fn actions come from `docs/PROTOCOL.md` |
| App screenshots | `src/assets/app-lighting-light.png` / `-dark.png`, the app on its demo data; the page shows the one matching the OS theme |
| Device tables (landing, `/docs/devices/`) | `devices/*.toml`, parsed at build time by `src/lib/devices.ts` |
| `/docs/protocol/` | a copy of `docs/PROTOCOL.md` with links rewritten; **update it by hand** when PROTOCOL.md changes |
| Measured numbers | typed in `src/pages/index.astro` and `src/content/docs/docs/faq.md`, from README.md / PRODUCT.md. Only measured numbers belong there. |

Docs pages live in `src/content/docs/docs/*.md`, except `/docs/devices/`, which is
`src/pages/docs/devices.astro` (a `StarlightPage`, so it can render the generated table) with its prose in
`src/docs-fragments/devices.md`. Write internal doc links root-relative (`/docs/configuration/`); a small
rehype plugin in `astro.config.mjs` adds the base path.

## Deploy

Static output, GitHub Pages friendly. By default the site is built for
`https://justinthevoid.github.io/uncoil/` (`site` + `base` in `astro.config.mjs`).

- **GitHub Pages:** `.github/workflows/deploy-site.yml` builds and deploys. It is manual-only while the repo
  is private; when it goes public, set Settings > Pages > Source to "GitHub Actions" and restore its push trigger.
- **Custom domain or another host:** build with `SITE` and `BASE` set, for example

  ```sh
  SITE=https://uncoil.example BASE=/ pnpm build
  ```

  or change the two defaults at the top of `astro.config.mjs`. For a GitHub Pages custom domain also add a
  `public/CNAME` file containing the domain.

## Open items

- [ ] Replace `src/assets/app-lighting-{light,dark}.png` (captures of the app on its demo data) with ones taken
      against the live engine.
- [ ] The phone video of the desk wave, once it is in the repo (a short muted loop would suit "The app").
- [ ] First GitHub release: the download button and install steps point at `/releases`; confirm the release
      ships `uncoild.exe` and `install-task.ps1` under those names.
- [ ] Domain, if any (see Deploy).
- [ ] Donation/sponsor links, if wanted (none on the site today).
- [ ] Social preview image (`og:image`) once there is a real screenshot.
