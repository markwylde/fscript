import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import fs from 'node:fs';

const fsGrammar = JSON.parse(fs.readFileSync('./fs.tmLanguage.json', 'utf-8'));

export default defineConfig({
	site: 'https://markwylde.github.io',
	base: '/fscript',
	integrations: [
		starlight({
			title: 'FScript',
			logo: {
				src: './src/assets/logo.svg',
			},
			social: {
				github: 'https://github.com/markwylde/fscript',
			},
			components: {
				SiteTitle: './src/components/SiteTitle.astro',
			},
			sidebar: [
				{
					label: 'Introduction',
					autogenerate: { directory: 'introduction' },
				},
				{
					label: 'Getting Started',
					autogenerate: { directory: 'getting-started' },
				},
				{
					label: 'Language Guide',
					autogenerate: { directory: 'language-guide' },
				},
				{
					label: 'Type System',
					autogenerate: { directory: 'type-system' },
				},
				{
					label: 'Standard Library',
					autogenerate: { directory: 'standard-library' },
				},
				{
					label: 'Runtime',
					autogenerate: { directory: 'runtime' },
				},
				{
					label: 'CLI',
					autogenerate: { directory: 'cli' },
				},
				{
					label: 'Examples',
					autogenerate: { directory: 'examples' },
				},
				{
					label: 'Implementation Status',
					autogenerate: { directory: 'implementation-status' },
				},
				{
					label: 'Reference',
					autogenerate: { directory: 'reference' },
				},
				{
					label: 'Playground',
					link: '/sandbox/',
				},
			],
			expressiveCode: {
				shiki: {
					langs: [fsGrammar],
					langAlias: {
						spec: 'fscript',
						'fscript-spec': 'fscript',
					},
				},
			},
		}),
	],
});
