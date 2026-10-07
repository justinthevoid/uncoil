<script lang="ts">
	// The lighting inside the PC that uncoil drives through OpenRGB (live mode), drawn where each part sits and
	// lit with the desk's effect. Placement is a best guess from the names OpenRGB reports (lib/pc.ts).
	import { onMount } from 'svelte';
	import { Pause, Play } from '@lucide/svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import PcInterior from '#lib/components/PcInterior.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import { createPreview } from '#lib/preview.svelte.ts';
	import { app } from '#lib/state.svelte.ts';
	import { pcParts, type PartKind } from '#lib/pc.ts';
	import { deskInputs, frameWith, hex } from '#lib/effect.ts';
	import { onTick } from '#lib/ticker.ts';
	import type { Config } from '#lib/types.ts';

	let { config }: { config: Config } = $props();
	const preview = createPreview(() => config);
	onMount(() => preview.start());

	const devices = $derived(app.status?.openrgb?.devices ?? []);
	const parts = $derived(pcParts(devices));
	const live = $derived(app.status?.openrgb?.state === 'connected');

	// Colours: the desk preview's, where the desk has the device; otherwise sampled here (the browser mock's
	// desk has no OpenRGB devices), down a column left of the keyboard like the desk's PC column.
	let t = $state(0);
	onMount(() => onTick((now) => (t = now)));
	function color(device: string, i: number): string | undefined {
		const di = preview.state.desk.findIndex((d) => d.id === device);
		if (di >= 0) return preview.state.colors[di]?.[i];
		const n = devices.findIndex((d) => d.id === device);
		const at = frameWith(config.effect, preview.state.paused ? 6 : t, config.saturation, config.brightness, deskInputs(preview.state.desk));
		return hex(at(device, '', -3 - n * 0.6, (i % 24) * 0.25));
	}

	let selected = $state<string | null>(null);
	const chosen = $derived(devices.find((d) => d.id === selected) ?? null);
	const WHAT: Record<PartKind, string> = { ram: 'Memory', gpu: 'Graphics card', board: 'Motherboard', fan: 'Fan', pump: 'Cooler pump', radiator: 'Radiator', other: 'Other lighting' };
	const chosenParts = $derived(parts.filter((p) => p.device === selected));
</script>

<Workspace title={pageTitle('pc')} subtitle="The lighting inside your PC that uncoil drives through OpenRGB, lit with the desk's effect. Where each part sits is a guess from its name." panelLabel="PC part">
	{#snippet tools()}
		<button class="btn-quiet" type="button" aria-pressed={preview.state.paused} onclick={() => (preview.state.paused = !preview.state.paused)}>
			{#if preview.state.paused}<Play size={14} />Play preview{:else}<Pause size={14} />Pause preview{/if}
		</button>
	{/snippet}

	{#if devices.length}
		<div class="stage-pc device-finish">
			<PcInterior {parts} {color} {selected} onselect={(id) => (selected = selected === id ? null : id)} />
		</div>
	{:else}
		<div class="empty">
			<h2 class="section-title">No PC lighting yet</h2>
			<p>uncoil lights your motherboard, memory, graphics card and fans through OpenRGB. Turn on live OpenRGB in Display &amp; RGB, and the parts OpenRGB finds show up here, in place, lit with the desk's effect.</p>
		</div>
	{/if}

	{#snippet panel()}
		{#if chosen}
			<div class="info">
				<h2 class="section-title">{chosen.name}</h2>
				<p class="note">{[...new Set(chosenParts.map((p) => WHAT[p.kind]))].join(', ')} · {chosen.leds} {chosen.leds === 1 ? 'light' : 'lights'}</p>
				<ul>
					{#each chosenParts as p (p.leds[0])}
						<li><span>{p.zone}</span><span class="n">{p.leds.length}</span></li>
					{/each}
				</ul>
				<p class="note">Give it its own effect on Lighting: choose the PC there, or pick its lights on the desk.</p>
			</div>
		{:else if devices.length}
			<p class="note">{live ? `${devices.length} ${devices.length === 1 ? 'device' : 'devices'} through OpenRGB. Click a part to see what it is.` : 'OpenRGB is not connected right now; the parts are what it reported last.'}</p>
		{:else}
			<p class="note">Nothing from OpenRGB yet.</p>
		{/if}
	{/snippet}
</Workspace>

<style>
	.stage-pc {
		width: 100%;
		max-width: 560px;
		height: min(70vh, 640px);
		margin: 0 auto;
		background: none;
	}
	.empty {
		display: grid;
		gap: 8px;
		max-width: 560px;
	}
	.empty p {
		margin: 0;
		color: var(--color-ink-2);
	}
	.info {
		display: grid;
		gap: 10px;
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
	}
	ul {
		display: grid;
		gap: 4px;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	li {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 6px 10px;
		border-radius: var(--radius);
		background: var(--color-surface);
	}
	.n {
		color: var(--color-ink-3);
		font-variant-numeric: tabular-nums;
	}
</style>
