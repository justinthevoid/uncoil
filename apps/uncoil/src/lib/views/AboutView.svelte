<script lang="ts">
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

<section class="about" aria-labelledby="about-title">
	<header class="head">
		<h1 id="about-title" class="title"><span class="display fac">FAC 400</span><span class="caps name">About</span></h1>
		<p class="lede">uncoil {s?.version ?? '0.1.0'} · GPL-3.0-or-later</p>
	</header>

	<div class="grid">
		<section class="block compare" aria-labelledby="weight-title">
			<h2 id="weight-title" class="caps sub">Weight</h2>
			<table>
				<thead>
					<tr class="caps-sm"><th></th><th scope="col">Synapse 4</th><th scope="col">uncoild</th></tr>
				</thead>
				<tbody>
					<tr><th scope="row" class="caps-sm">Processes</th><td class="num">17</td><td class="num">1</td></tr>
					<tr><th scope="row" class="caps-sm">Memory</th><td class="num">~1.4 GB, growing</td><td class="num">{s?.memory_bytes ? `${mb(s.memory_bytes)} MB, now` : '~3 MB'}</td></tr>
					<tr><th scope="row" class="caps-sm">Install</th><td class="num">~500 MB</td><td class="num">{s?.exe_bytes ? `${Math.round(s.exe_bytes / 1024)} KB` : '651 KB'}</td></tr>
					<tr><th scope="row" class="caps-sm">Drivers</th><td>Kernel filter drivers</td><td>None, user-mode HID</td></tr>
				</tbody>
			</table>
			<p class="note">Synapse figures measured on the maintainer's PC. uncoild figures are live when the engine is running.</p>
		</section>

		<section class="block" aria-labelledby="what-title">
			<h2 id="what-title" class="caps sub">What it is</h2>
			<p>A small background engine that drives Razer lighting from one shared effect field, and this app to adjust it. Close the app whenever you like; the engine keeps running on its own.</p>
			<p>Features written into the devices themselves, like Fn+P for Print Screen, keep working even without uncoil.</p>
			<div class="actions">
				<button type="button" class="btn" onclick={() => open(REPO)}><span class="caps-sm">Source code</span><span class="block-red" aria-hidden="true"></span></button>
				<button type="button" class="btn ghost" onclick={() => open(`${REPO}/blob/main/docs/PROTOCOL.md`)}><span class="caps-sm">How it works</span></button>
			</div>
		</section>

		<section class="block wide" aria-labelledby="credit-title">
			<h2 id="credit-title" class="caps sub">Credits</h2>
			<p>Protocol knowledge from <strong>OpenRazer</strong>, <strong>OpenRGB</strong> and <strong>OpenSynapse</strong>; colour from <strong>FastLED</strong>'s rainbow. Not affiliated with or endorsed by Razer Inc. Razer, Synapse and Chroma are trademarks of Razer Inc.</p>
		</section>
	</div>
</section>

<style>
	.about {
		display: grid;
		grid-template-rows: auto 1fr;
		height: 100%;
		border: var(--hair);
		overflow: auto;
	}
	.head {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: 24px;
		padding: 22px 24px 18px;
		border-bottom: var(--hair);
	}
	.title {
		display: flex;
		align-items: baseline;
		gap: 18px;
		margin: 0;
		font-weight: inherit;
	}
	.fac {
		font-size: 44px;
	}
	.name {
		color: var(--color-ink-2);
		letter-spacing: 0.32em;
	}
	.lede {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		letter-spacing: 0.06em;
	}
	.grid {
		display: grid;
		grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
		align-content: start;
	}
	.block {
		display: grid;
		gap: 14px;
		align-content: start;
		padding: 24px;
		border-bottom: var(--hair);
	}
	.compare {
		border-right: var(--hair);
	}
	.wide {
		grid-column: 1 / -1;
	}
	.sub {
		margin: 0;
		color: var(--color-ink-2);
		font-weight: 500;
	}
	p {
		margin: 0;
		color: var(--color-ink-2);
		font-size: 13px;
		line-height: 1.6;
		max-width: 62ch;
	}
	strong {
		color: var(--color-ink);
		font-weight: 500;
	}
	table {
		border-collapse: collapse;
		width: 100%;
	}
	th,
	td {
		text-align: left;
		padding: 11px 12px 11px 0;
		border-bottom: var(--hair);
		font-weight: 400;
	}
	td {
		font-size: 13px;
	}
	thead th {
		white-space: nowrap;
	}
	thead th {
		color: var(--color-ink-3);
		border-bottom: var(--hair-strong);
	}
	tbody th {
		color: var(--color-ink-3);
	}
	td:last-child {
		color: var(--color-ink);
	}
	td:nth-child(2) {
		color: var(--color-ink-3);
	}
	.note {
		font-size: 12px;
		color: var(--color-ink-3);
	}
	.actions {
		display: flex;
		gap: 10px;
		margin-top: 4px;
	}
	.btn {
		position: relative;
		display: flex;
		align-items: center;
		gap: 14px;
		height: 38px;
		padding: 0 34px 0 16px;
		border: var(--hair-ink);
		background: none;
		color: var(--color-ink);
		overflow: hidden;
	}
	.btn.ghost {
		padding-right: 16px;
		border: var(--hair-strong);
		color: var(--color-ink-2);
	}
	.block-red {
		position: absolute;
		right: 0;
		top: 0;
		bottom: 0;
		width: 26px;
		background: var(--color-fac-red);
		transform: scaleX(0.6923);
		transform-origin: right;
		transition: transform var(--t-mid) var(--ease);
	}
	.btn:hover .block-red {
		transform: none;
	}
	.btn.ghost:hover {
		color: var(--color-ink);
		border-color: var(--color-ink-3);
	}
</style>
