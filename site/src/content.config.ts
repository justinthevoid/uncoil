import { defineCollection } from 'astro:content';
import { z } from 'astro/zod';
import { docsLoader, i18nLoader } from '@astrojs/starlight/loaders';
import { docsSchema, i18nSchema } from '@astrojs/starlight/schema';

export const collections = {
	docs: defineCollection({
		loader: docsLoader(),
		// `fac`: the page's catalog number, set inline before the title (never as a kicker above it).
		schema: docsSchema({ extend: z.object({ fac: z.string().optional() }) }),
	}),
	// UI string overrides (src/content/i18n/en.json).
	i18n: defineCollection({ loader: i18nLoader(), schema: i18nSchema() }),
};
