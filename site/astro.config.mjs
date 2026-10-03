// @ts-check
import { defineConfig } from 'astro/config';
import { fileURLToPath } from 'node:url';
import starlight from '@astrojs/starlight';
import svelte from '@astrojs/svelte';
import tailwindcss from '@tailwindcss/vite';

// Where the site is served from. Defaults to the GitHub Pages project site
// https://justinthevoid.github.io/uncoil/. For a custom domain, build with
// SITE=https://example.org BASE=/ (or edit the two defaults below). See README.md.
const site = process.env.SITE ?? 'https://justinthevoid.github.io';
const base = process.env.BASE ?? '/uncoil';
const repo = 'https://github.com/justinthevoid/uncoil';

/** Markdown links written root-relative (`/docs/devices/`) get the base prefix, so docs stay portable. */
function rehypeBaseLinks() {
	const prefix = base.replace(/\/+$/, '');
	/** @param {any} node */
	const visit = (node) => {
		const href = node.type === 'element' && node.tagName === 'a' ? node.properties?.href : undefined;
		if (typeof href === 'string' && href.startsWith('/') && !href.startsWith('//') && !href.startsWith(prefix + '/')) {
			node.properties.href = prefix + href;
		}
		node.children?.forEach(visit);
	};
	return /** @param {any} tree */ (tree) => visit(tree);
}

export default defineConfig({
	site,
	base,
	trailingSlash: 'ignore',
	markdown: { rehypePlugins: [rehypeBaseLinks] },
	integrations: [
		svelte(),
		starlight({
			title: 'uncoil',
			description:
				'Documentation for uncoil, a tiny open-source lighting daemon for Razer peripherals on Windows.',
			favicon: '/favicon.svg',
			social: [{ icon: 'github', label: 'uncoil on GitHub', href: repo }],
			editLink: { baseUrl: `${repo}/edit/main/site/` },
			credits: false,
			// The 404 page is the site's own (src/pages/404.astro), in the landing page's layout.
			disable404Route: true,
			lastUpdated: false,
			customCss: ['@fontsource-variable/archivo/wdth.css', './src/styles/starlight.css'],
			components: {
				SiteTitle: './src/components/docs/SiteTitle.astro',
				PageTitle: './src/components/docs/PageTitle.astro',
				ThemeProvider: './src/components/docs/ThemeProvider.astro',
				ThemeSelect: './src/components/docs/ThemeSelect.astro',
			},
			expressiveCode: {
				themes: ['github-dark-default'],
				// Plain frames: no faux window chrome on shell snippets.
				defaultProps: { frame: 'code' },
				styleOverrides: {
					borderRadius: '0',
					borderColor: '#262626',
					borderWidth: '1px',
					codeBackground: '#111111',
					codeFontFamily: "'Cascadia Mono', ui-monospace, Consolas, monospace",
					uiFontFamily: "'Archivo Variable', 'Segoe UI', system-ui, sans-serif",
					frames: {
						editorTabBarBackground: '#0b0b0b',
						editorActiveTabBackground: '#111111',
						editorActiveTabIndicatorTopColor: '#e2372c',
						editorActiveTabIndicatorBottomColor: 'transparent',
						terminalTitlebarBackground: '#0b0b0b',
						terminalBackground: '#111111',
						terminalTitlebarBorderBottomColor: '#262626',
						frameBoxShadowCssValue: 'none',
					},
				},
			},
			sidebar: [
				{
					label: 'Use',
					items: [
						{ label: 'Overview', slug: 'docs' },
						'docs/getting-started',
						'docs/configuration',
						'docs/troubleshooting',
						'docs/faq',
					],
				},
				{ label: 'Hardware', items: [{ label: 'Devices', link: '/docs/devices/' }, 'docs/protocol'] },
				{ label: 'Project', items: ['docs/architecture', 'docs/contributing'] },
			],
		}),
	],
	vite: {
		plugins: [tailwindcss()],
		resolve: {
			// The landing page's pulse plot runs the desktop app's own effect maths and desk snapshot.
			alias: { $uncoil: fileURLToPath(new URL('../apps/uncoil/src/lib', import.meta.url)) },
		},
		// ../apps (effect maths, desk snapshot) and ../devices (TOML data) live outside the site root.
		server: { fs: { allow: ['..'] } },
	},
});
