# uncoil site

The website and documentation for uncoil: a landing page at `/` and Starlight docs at `/docs/`, built as one
static Astro project.

- Astro 7 and Starlight (docs). No client framework and no CSS framework: the landing page's one script is the
  film (`src/film/film.ts`, about 24 KB gzipped with the desk data and effect maths).
- Landing page: "Uncoiling", its own small world. Always dark; the only light is the desk's 113 LEDs and a
  brass lamp; one canvas behind sticky scenes, where the same 113 points start as the uncoil spiral, unwind
  onto their measured positions and morph from scene to scene, moved by scroll through a spring. Its world
  rules are at the top of `src/pages/index.astro`. Type: Fraunces (self-hosted from `@fontsource-variable`)
  with the system mono for data.
- One world for every page: `src/styles/world.css` holds the tokens (dark, bone ink, brass), the font and
  the shared wordmark and buttons, imported by the landing page, the 404 page and `src/styles/starlight.css`,
  which maps Starlight's colours onto it. The docs are always dark (`ThemeProvider.astro` pins `data-theme`;
  there is no picker); the current page in the sidebar is the one lit LED. The 404 page is the coil again,
  one light short.
- The desktop app keeps its own design system, the Swatch Book (`DESIGN.md`); the site no longer mirrors it.
- No third-party requests anywhere.
- Reduced motion: every scene shows a still frame (rendered at build time from the same geometry, so it also
  works without JavaScript) and the light waits for Play.

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
| The desk and its light (every landing scene) | `src/lib/desk.ts` flattens the app's desk snapshot (`apps/uncoil/src/lib/mock/desk.json`) into 113 LEDs; colours come from the app's own `apps/uncoil/src/lib/effect.ts` (through the `$uncoil` alias in `astro.config.mjs`) |
| Device drawings | `src/art/`: the app's traced device art (`apps/uncoil/src/lib/art/`: keyboard cases, the Basilisk V3 Pro's traced outline and seams, the mat's edge) built into desk geometry once (`geometry.ts`), drawn on canvas per frame (`renderer.ts`) and as build-time SVG stills (`svg.ts`). Change the art in the app, not here |
| The film | `src/film/film.ts` (scenes, camera, springs; the `MOTION` table holds every easing and timing); `src/film/still.ts` renders each scene's no-JavaScript / reduced-motion still. Debug: `?y=<px>&t=<s>&debug`, documented at the top of film.ts |
| Fn layer with gel tabs ("Keeps its own brain") | the keyboard's real layout from desk.json; the Fn actions come from `docs/PROTOCOL.md` |
| App screenshot | `src/assets/app-lighting-dark.png`, the app on its demo data |
| Device lists (landing: tested and experimental, `/docs/devices/`) | `devices/*.toml` and `devices/experimental/*.toml`, parsed at build time by `src/lib/devices.ts`; the counts on the page are computed, never typed |
| `/docs/protocol/` | a copy of `docs/PROTOCOL.md` with links rewritten; **update it by hand** when PROTOCOL.md changes |
| Measured numbers | `MEASURED` in `src/lib/desk.ts` and the copy in `src/pages/index.astro`, plus `src/content/docs/docs/faq.md`, from README.md / PRODUCT.md. Only measured numbers belong there. |

Docs pages live in `src/content/docs/docs/*.md`, except `/docs/devices/`, which is
`src/pages/docs/devices.astro` (a `StarlightPage`, so it can render the generated table) with its prose in
`src/docs-fragments/devices.md`. Write internal doc links root-relative (`/docs/configuration/`); a small
rehype plugin in `astro.config.mjs` adds the base path.

## Deploy

Static output, built for `https://uncoil.justinthevoid.com/` (`site` + `base` in `astro.config.mjs`) and
served by GitHub Pages with that custom domain.

- **GitHub Pages:** `.github/workflows/deploy-site.yml` builds and deploys on pushes that change the site. Its
  jobs skip themselves while the repository is private. Settings > Pages: Source "GitHub Actions", custom
  domain `uncoil.justinthevoid.com`, Enforce HTTPS. DNS: a `CNAME` record `uncoil` pointing at
  `justinthevoid.github.io` (verify the domain under the account's Pages settings, so no one else can claim
  it). With an Actions deployment the domain lives in the settings, not in a `CNAME` file.
- **Another host or address:** build with `SITE` and `BASE` set, for example

  ```sh
  SITE=https://justinthevoid.github.io BASE=/uncoil pnpm build
  ```

## Open items

- [x] `src/assets/app-lighting-dark.png` recaptured on 2026-10-07 (the app on its demo data, headless
      Chrome, nothing else in frame). A capture against the live engine would show real devices.
- [ ] The phone video of the desk wave, once it is in the repo (a short muted loop would suit "The app").
- [x] Install steps: download from `/releases` (the release ships `uncoild.exe`, `uncoil.exe`,
      `install-task.ps1`, `uninstall-task.ps1`, the app installer and `SHA256SUMS.txt`), and `install-task.ps1`
      installs the `uncoild.exe` next to it. Check on the first release that the names still match.
- [ ] Domain, if any (see Deploy).
- [ ] Donation/sponsor links, if wanted (none on the site today).
- [ ] Social preview image (`og:image`) once there is a real screenshot.
