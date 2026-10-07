// @ts-check
import { defineConfig } from 'astro/config';
import { fileURLToPath } from 'node:url';
import starlight from '@astrojs/starlight';
import { unified } from '@astrojs/markdown-remark';

// Where the site is served from: https://uncoil.justinthevoid.com/ (GitHub Pages with a custom domain). To
// build it for somewhere else, set SITE and BASE, e.g. SITE=https://justinthevoid.github.io BASE=/uncoil for
// the plain project-site address. See README.md.
const site = process.env.SITE ?? 'https://uncoil.justinthevoid.com';
const base = process.env.BASE ?? '/';
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
	markdown: { processor: unified({ rehypePlugins: [rehypeBaseLinks] }) },
	integrations: [
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
			customCss: ['./src/styles/starlight.css'],
			components: {
				SiteTitle: './src/components/docs/SiteTitle.astro',
				PageTitle: './src/components/docs/PageTitle.astro',
				ThemeProvider: './src/components/docs/ThemeProvider.astro',
				ThemeSelect: './src/components/docs/ThemeSelect.astro',
			},
			// Starlight's own code themes read the --sl-color-* values (mapped to the world's tokens in starlight.css);
			// the docs are always dark, so the dark one is the one that shows.
			expressiveCode: {
				// Plain frames: no faux window chrome on shell snippets.
				defaultProps: { frame: 'code' },
				styleOverrides: {
					borderRadius: '10px',
					borderColor: 'var(--hair)',
					codeFontFamily: "ui-monospace, 'Cascadia Mono', 'Segoe UI Mono', Consolas, monospace",
					uiFontFamily: "ui-monospace, 'Cascadia Mono', 'Segoe UI Mono', Consolas, monospace",
					codeBackground: 'var(--raise)',
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
		resolve: {
			// The landing page's desk runs the desktop app's own effect maths on its desk snapshot.
			alias: { $uncoil: fileURLToPath(new URL('../apps/uncoil/src/lib', import.meta.url)) },
		},
		// ../apps (effect maths, desk snapshot) and ../devices (TOML data) live outside the site root.
		server: { fs: { allow: ['..'] } },
	},
});
