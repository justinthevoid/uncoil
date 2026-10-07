// See https://svelte.dev/docs/kit/types#app.d.ts
declare global {
	namespace App {}
	/** The app's version, from package.json (vite.config.ts). */
	const __APP_VERSION__: string;
}

export {};
