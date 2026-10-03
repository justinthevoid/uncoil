<script lang="ts">
	import { ExternalLink } from '@lucide/svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import { inTauri } from '#lib/api.ts';
	import { app } from '#lib/state.svelte.ts';

	const REPO = 'https://github.com/justinthevoid/uncoil';
	async function open(url: string) {
		if (inTauri) {
			const { openUrl } = await import('@tauri-apps/plugin-opener');
			await openUrl(url);
		} else window.open(url, '_blank', 'noopener');
	}
	const s = $derived(app.status);
	const mb = (b: number) => (b / 1048576).toFixed(1);
</script>

<Workspace title="About uncoil" subtitle="Version {s?.version ?? '0.1.0'} · free and open source (GPL-3.0-or-later)">
	<section class="card">
		<p class="lead">A small background engine that runs your Razer lighting, and this app to adjust it. Close the app whenever you like; the engine keeps running on its own.</p>
		<p>Changes saved into the devices themselves, like Fn+P for Print Screen, keep working even without uncoil.</p>
		<div class="actions">
			<button type="button" class="btn" onclick={() => open(REPO)}>Source code<ExternalLink size={14} /></button>
			<button type="button" class="btn-quiet" onclick={() => open(`${REPO}/blob/main/docs/PROTOCOL.md`)}>How it works<ExternalLink size={14} /></button>
		</div>
	</section>

	<section class="card" aria-labelledby="cmp-title">
		<h2 id="cmp-title" class="section-title">Compared with Synapse</h2>
		<table>
			<thead><tr><th></th><th scope="col">Synapse 4</th><th scope="col">uncoil engine</th></tr></thead>
			<tbody>
				<tr><th scope="row">Processes</th><td class="num">17</td><td class="num">1</td></tr>
				<tr><th scope="row">Memory</th><td class="num">~1.4 GB, growing</td><td class="num">{s?.memory_bytes ? `${mb(s.memory_bytes)} MB right now` : '~3 MB'}</td></tr>
				<tr><th scope="row">Install size</th><td class="num">~500 MB</td><td class="num">{s?.exe_bytes ? `${Math.round(s.exe_bytes / 1024)} KB` : 'under 1 MB'}</td></tr>
				<tr><th scope="row">Drivers</th><td>Kernel filter drivers</td><td>None</td></tr>
			</tbody>
		</table>
		<p class="note">Synapse figures measured on the maintainer's PC. uncoil's are live while the engine runs.</p>
	</section>

	<p class="credits">Built on what OpenRazer, OpenRGB and OpenSynapse learned about these devices; colours use FastLED's rainbow. Not affiliated with or endorsed by Razer Inc. Razer, Synapse and Chroma are trademarks of Razer Inc.</p>
</Workspace>

<style>
	.card {
		display: grid;
		gap: 12px;
		max-width: 720px;
		padding: 18px 20px;
		border: var(--hair);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
	}
	p {
		margin: 0;
		color: var(--color-ink-2);
		line-height: 1.55;
		max-width: 66ch;
	}
	.lead {
		color: var(--color-ink);
		font-size: 14px;
	}
	.actions {
		display: flex;
		gap: 8px;
		margin-top: 4px;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th,
	td {
		text-align: left;
		padding: 9px 12px 9px 0;
		border-bottom: var(--hair);
		font-weight: 400;
	}
	thead th {
		color: var(--color-ink-3);
		font-size: 12px;
		font-weight: 500;
	}
	tbody th {
		color: var(--color-ink-3);
	}
	tbody tr:last-child > * {
		border-bottom: 0;
	}
	td:nth-child(2) {
		color: var(--color-ink-3);
	}
	td:last-child {
		font-weight: 600;
	}
	.note,
	.credits {
		color: var(--color-ink-3);
		font-size: 12px;
	}
</style>
