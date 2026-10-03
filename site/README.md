# uncoil site

The website and documentation for uncoil: a landing page at `/` and Starlight docs at `/docs/`, built as one
static Astro project.

- Astro 7, Svelte 5 (the pulse-plot island), Tailwind CSS 4 (landing page), Starlight (docs).
- Same design tokens as the desktop app (`apps/uncoil/src/app.css`): `src/styles/site.css` for the landing
  page, `src/styles/tokens.css` + `src/styles/starlight.css` for the docs. Change all three together.
- Fonts are self-hosted (`@fontsource-variable/archivo`, width axis). No third-party requests.

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
| Pulse plot (landing hero) | `src/components/PulsePlot.svelte`, running the app's own `apps/uncoil/src/lib/effect.ts` on `apps/uncoil/src/lib/mock/desk.json` (imported through the `$uncoil` alias in `astro.config.mjs`) |
| Desk map (landing, FAC 02) | `src/components/DeskMap.astro`, same effect and desk, rendered at build time |
| Device tables (landing, `/docs/devices/`) | `devices/*.toml`, parsed at build time by `src/lib/devices.ts` |
| `/docs/protocol/` | a copy of `docs/PROTOCOL.md` with links rewritten; **update it by hand** when PROTOCOL.md changes |
| Measured numbers | typed in `src/pages/index.astro` and `src/content/docs/docs/faq.md`, from README.md / PRODUCT.md. Only measured numbers belong there. |

Docs pages live in `src/content/docs/docs/*.md`, except `/docs/devices/`, which is
`src/pages/docs/devices.astro` (a `StarlightPage`, so it can render the generated table) with its prose in
`src/docs-fragments/devices.md`. Write internal doc links root-relative (`/docs/configuration/`); a small
rehype plugin in `astro.config.mjs` adds the base path.

Each doc page sets `fac:` in its frontmatter: its catalog number, shown inline before the title.

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

- [ ] Replace `src/assets/app-lighting.png` (a real capture of the app on its demo data) with one taken
      against the live engine.
- [ ] The phone video of the desk wave, once it is in the repo (a short muted loop would suit FAC 03).
- [ ] First GitHub release: the download button and install steps point at `/releases`; confirm the release
      ships `uncoild.exe` and `install-task.ps1` under those names.
- [ ] Domain, if any (see Deploy).
- [ ] Donation/sponsor links, if wanted (none on the site today).
- [ ] Social preview image (`og:image`) once there is a real screenshot.
