<script lang="ts">
	import { onMount } from 'svelte';
	import { ChevronRight, ExternalLink } from '@lucide/svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import { getDesk, openExternal } from '#lib/api.ts';
	import { app, deskSources } from '#lib/state.svelte.ts';
	import { pipe, loadDevices, shortName } from '#lib/daemon.svelte.ts';
	import { EXPERIMENTAL, EXPERIMENTAL_TEXT, SUPPORT_URL, productId } from '#lib/checks.ts';
	import type { Config, DeskDevice, DeviceKind } from '#lib/types.ts';

	let { config, onopen }: { config: Config; onopen: (id: string) => void } = $props();

	let desk = $state<DeskDevice[]>([]);
	onMount(async () => {
		const src = deskSources();
		desk = await getDesk($state.snapshot(config) as Config, src.connected, src.external);
		if (!pipe.loaded) loadDevices();
	});

	const kind: Record<string, string> = { keyboard: 'Keyboard', mouse: 'Mouse', mousemat: 'Mouse mat', headset: 'Headset', other: 'Device' };
	// Desk devices, then connected devices with no lighting layout (they have no place on the desk).
	const all = $derived([
		...desk.filter((d) => !d.id.startsWith('openrgb:')).map((d) => ({ id: d.id, name: d.name, kind: d.kind as DeviceKind, leds: (d.kind === 'mousemat' ? 1 : d.shapes.length) as number | null })),
		...pipe.devices.filter((p) => !desk.some((d) => d.id === p.id)).map((p) => ({ id: p.id, name: p.name, kind: p.kind, leds: null }))
	]);
	const rows = $derived(
		all.map((d) => {
			const s = app.status?.devices.find((x) => x.id === d.id);
			return {
				id: d.id,
				name: shortName(d.name),
				kind: kind[d.kind] ?? 'Device',
				experimental: pipe.devices.find((p) => p.id === d.id)?.support === 'experimental',
				live: !!s,
				errors: s?.errors ?? 0,
				link: s?.connection ? s.connection[0].toUpperCase() + s.connection.slice(1) : '—',
				leds: d.leds
			};
		})
	);
	const unknown = $derived(app.status?.unknown_devices ?? []);
	const connected = $derived(rows.filter((r) => r.live).length);

	// The PC's other lighting, driven live through OpenRGB (Settings turns it on).
	const orgb = $derived(app.status?.openrgb ?? null);
	const orgbRows = $derived((orgb?.devices ?? []).map((d) => ({ ...d, onDesk: desk.some((x) => x.id === d.id) })));
	const orgbText = $derived.by(() => {
		if (!orgb || orgb.state === 'off') return '';
		if (orgb.state === 'connected') {
			const n = orgbRows.length;
			const where = n && orgbRows.every((r) => !r.onDesk) ? ' They aren’t drawn on the desk preview yet.' : '';
			return n ? `uncoil sends the desk effect to ${n === 1 ? 'this device' : `these ${n} devices`} through OpenRGB.${where}` : 'OpenRGB is connected but reports no devices besides Razer ones, which uncoil drives itself.';
		}
		if (orgb.state === 'waiting') return orgb.detail ?? 'Waiting for OpenRGB to start.';
		return orgb.detail ?? 'uncoil couldn’t talk to OpenRGB.';
	});
</script>

<Workspace
	title={pageTitle('devices')}
	subtitle={app.status ? `${connected} of ${rows.length} connected. Unplugged devices are picked up again within a few seconds.` : 'The engine isn’t running, so connection status is unknown.'}
>
	<div class="table" role="table" aria-label="Devices">
		<div class="tr th" role="row">
			<span role="columnheader">Device</span><span role="columnheader">Status</span><span role="columnheader">Link</span><span role="columnheader" class="r">LEDs</span><span></span>
		</div>
		{#each rows as r (r.id)}
			<!-- a real button in the first cell, stretched over the row, so the row keeps its table role -->
			<div class="tr link" role="row">
				<span role="cell"><button type="button" class="open" onclick={() => onopen(r.id)}><b>{r.name}</b></button><small>{r.kind}{#if r.experimental}<span class="exp" title={EXPERIMENTAL_TEXT}>{EXPERIMENTAL}</span>{/if}</small></span>
				<span role="cell" class="status"><span class="dot" class:on={r.live} class:warn={r.errors > 0}></span>{!app.status ? 'Unknown' : !r.live ? 'Not connected' : r.errors ? `${r.errors} error${r.errors === 1 ? '' : 's'}` : 'Connected'}</span>
				<span role="cell">{r.link}</span>
				<span role="cell" class="r num">{r.leds ?? 'None'}</span>
				<span class="go" aria-hidden="true"><ChevronRight size={16} /></span>
			</div>
		{/each}
		{#each unknown as u (u.product_id)}
			<div class="tr unknown" role="row">
				<span role="cell" class="span"><b>A Razer device uncoil doesn't know yet (product ID {productId(u.product_id)}).</b><small>It's connected, but there's no device file for it, so uncoil leaves it alone.</small></span>
				<span role="cell" class="ask"><button type="button" class="btn-quiet" onclick={() => openExternal(SUPPORT_URL)}>Ask for support<ExternalLink size={14} /></button></span>
			</div>
		{/each}
	</div>

	{#if orgb && orgb.state !== 'off'}
		<section class="orgb" aria-labelledby="orgb-title">
			<h2 id="orgb-title" class="section-title">Through OpenRGB</h2>
			<p class="sub" class:bad={orgb.state === 'error'}>{orgbText}</p>
			{#if orgbRows.length}
				<div class="table" role="table" aria-label="Devices through OpenRGB">
					<div class="tr th orgb-row" role="row">
						<span role="columnheader">Device</span><span role="columnheader">Status</span><span role="columnheader">Desk</span><span role="columnheader" class="r">LEDs</span>
					</div>
					{#each orgbRows as r (r.id)}
						<div class="tr orgb-row" role="row">
							<span role="cell"><b>{r.name}</b><small>PC lighting</small></span>
							<span role="cell" class="status"><span class="dot" class:on={orgb.state === 'connected'}></span>{orgb.state === 'connected' ? 'Connected' : 'Not connected'}</span>
							<span role="cell">{r.onDesk ? 'On the desk' : 'Listed here only'}</span>
							<span role="cell" class="r num">{r.leds}</span>
						</div>
					{/each}
				</div>
			{/if}
		</section>
	{/if}
</Workspace>

<style>
	.table {
		display: grid;
		max-width: 900px;
		border: var(--hair);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		overflow: hidden;
	}
	.tr {
		display: grid;
		grid-template-columns: minmax(0, 2fr) minmax(0, 1.2fr) minmax(0, 1fr) 60px 28px;
		align-items: center;
		gap: 12px;
		min-height: 56px;
		padding: 0 14px 0 18px;
		border: 0;
		border-top: var(--hair);
		background: none;
		text-align: left;
	}
	.th {
		min-height: 38px;
		border-top: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		font-weight: 500;
	}
	.link {
		position: relative;
	}
	.link:hover {
		background: var(--color-surface-2);
	}
	.open {
		padding: 0;
		border: 0;
		background: none;
		color: inherit;
		text-align: left;
	}
	.open::after {
		content: '';
		position: absolute;
		inset: 0;
	}
	.open:focus-visible {
		outline: none;
	}
	.link:has(.open:focus-visible) {
		outline: 2px solid var(--color-select);
		outline-offset: -2px;
	}
	b {
		display: block;
		font-weight: 600;
	}
	small {
		color: var(--color-ink-3);
		font-size: 12px;
	}
	.status {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--color-ink-4);
	}
	.dot.on {
		background: var(--color-ok);
	}
	.dot.warn {
		background: var(--color-warn);
	}
	.r {
		text-align: right;
	}
	.go {
		color: var(--color-ink-4);
	}
	small {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.exp {
		position: relative;
		z-index: 1;
		padding: 0 6px;
		border: var(--hair-strong);
		border-radius: var(--radius-sm);
		color: var(--color-ink-2);
		font-size: 11px;
		font-weight: 500;
		line-height: 16px;
	}
	.orgb {
		display: grid;
		gap: 8px;
		max-width: 900px;
	}
	.orgb .section-title {
		margin: 0;
	}
	.sub {
		margin: 0 0 4px;
		color: var(--color-ink-3);
		font-size: 12px;
	}
	.sub.bad {
		color: var(--color-warn);
	}
	.orgb-row {
		grid-template-columns: minmax(0, 2fr) minmax(0, 1.2fr) minmax(0, 1fr) 60px 28px;
	}
	.unknown {
		grid-template-columns: minmax(0, 1fr) auto;
		padding: 10px 14px 10px 18px;
	}
	.unknown b {
		font-weight: 500;
	}
	.ask {
		justify-self: end;
	}
</style>
