# uncoil (desktop app)

The settings window for uncoil: a live preview of your desk lighting plus the controls for it. It edits
`%APPDATA%\uncoil\config.json`, which the engine (`uncoild`) hot-reloads, and reads the engine's
`%LOCALAPPDATA%\uncoil\status.json` for the Devices page. The preview is drawn from `uncoil-core`'s real
effect code, so what you see is exactly what the engine sends to the hardware.

Stack: Tauri 2, SvelteKit 3 (static SPA) with Svelte 5, Tailwind CSS 4, pnpm.

## Develop

```sh
pnpm install
pnpm tauri dev      # app window with hot reload (starts the Vite dev server on :1420)
```

`pnpm dev` on its own serves the UI in a normal browser at http://localhost:1420. Outside Tauri the
backend calls go to a mock (`src/lib/mock/`) with the real desk geometry and a TypeScript port of the
effect maths, which is handy for design work. If `uncoil-core`'s device layouts change, refresh the mock:

```sh
UNCOIL_UPDATE_MOCK=1 cargo test -p uncoil-gui   # from the repo root; plain `cargo test` checks it
```

## Build

```sh
pnpm tauri build    # release build plus NSIS installer under target/release/bundle/
```

Other checks: `pnpm check` (svelte-check / TypeScript), `pnpm build` (frontend only, into `build/`),
`cargo build -p uncoil-gui` (Rust side only, from the repo root).

## Layout

- `src/routes/+page.svelte`: window shell (navigation, engine status, debounced config saving)
- `src/lib/views/`: Lighting, Devices, Display, About
- `src/lib/components/DeskPreview.svelte`: the canvas preview, about 30 frames a second
- `src/lib/api.ts`: typed wrappers over the Tauri commands
- `src-tauri/src/main.rs`: the commands `get_config`, `save_config`, `get_desk`, `preview_frame`, `get_status`
