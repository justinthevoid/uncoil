// Tauri has no Node server, so render everything client-side. With `ssr = false` there is nothing to
// prerender: adapter-static writes a single SPA shell (fallback `index.html`, see vite.config.ts)
// that Tauri serves for every route. This is the setup from Tauri's official SvelteKit guide.
export const ssr = false;
