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
	import OptionList from '#lib/components/OptionList.svelte';
	import type { Config, OpenRgbLive } from '#lib/types.ts';

	let { config }: { config: Config } = $props();
	const preview = createPreview(() => config);
	onMount(() => preview.start());

	const devices = $derived(app.status?.openrgb?.devices ?? []);
	// devices a running program (iCUE, Armoury Crate) lights itself; uncoil takes them back when it quits
	const held = $derived(app.status?.openrgb?.held ?? []);
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

	// `openrgb.live`, changed one key at a time so the others (port, exclude, pins) stay as they are
	function setLive(patch: Partial<OpenRgbLive>) {
		const o = config.openrgb ?? { devices: [] };
		config.openrgb = { ...o, live: { port: 6742, exclude: [], ...o.live, ...patch } };
	}
	const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

	// `openrgb.live.exclude`: devices left to their own software. The daemon stops lighting them, and uncoil's
	// OpenRGB turns off the detector by that name where there is one, so their own app can take them back.
	const excluded = $derived(config.openrgb?.live?.exclude ?? []);
	function setExcluded(exclude: string[]) {
		setLive({ exclude });
	}
	function leave(name: string) {
		if (!excluded.some((e) => same(e, name))) setExcluded([...excluded, name]);
		selected = null;
	}

	// `openrgb.live.pins`: who lights a device, over the daemon's own judgement (uncoil_core::owners). As in the
	// engine, the first pin whose `match` is part of the device's name decides.
	const pins = $derived(config.openrgb?.live?.pins ?? []);
	const programs = $derived(app.status?.openrgb?.programs ?? []);
	const pinFor = (name: string) => pins.find((p) => p.match.trim() && name.toLowerCase().includes(p.match.trim().toLowerCase()));
	function choose(name: string, to: string) {
		const rest = pins.filter((p) => !same(p.match, name));
		setLive({ pins: to === 'auto' ? rest : [...rest, { match: name, to }] });
	}
	const whoLabel = (to: string) => (same(to, 'uncoil') ? 'uncoil, always' : `${to}, while it runs`);
	const whoOptions = $derived([
		{ value: 'auto', label: 'Automatic' },
		{ value: 'uncoil', label: 'uncoil, always' },
		...programs.map((p) => ({ value: p, label: `${p}, while it runs` }))
	]);
	const chosenWho = $derived.by(() => {
		const pin = chosen ? pinFor(chosen.name) : undefined;
		if (!pin) return 'auto';
		return same(pin.to, 'uncoil') ? 'uncoil' : (programs.find((p) => same(p, pin.to)) ?? 'auto');
	});
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
	{:else if held.length}
		<div class="empty">
			<h2 class="section-title">Other software has the PC's lighting</h2>
			<p>Everything OpenRGB found is lit by {[...new Set(held.map((h) => h.by))].join(' and ')} right now. uncoil lights it again a few seconds after that quits.</p>
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
				<div class="who">
					<h3 class="section-title">Who lights it</h3>
					<OptionList label="Who lights it" options={whoOptions} value={chosenWho} onchange={(v) => choose(chosen.name, v)} />
					<p class="note">Automatic: uncoil, except while a program known to light it runs. A program you pick has it only while it runs; uncoil lights it the rest of the time.</p>
				</div>
				<div class="leave">
					<button type="button" class="btn-quiet" onclick={() => leave(chosen.name)}>Leave it to its own software</button>
					<p class="note">uncoil stops lighting it, and OpenRGB lets go of it where it can (it restarts, a second or two dark), so iCUE or the maker's app can take it back. Every device with this name goes.</p>
				</div>
			</div>
		{:else if devices.length}
			<p class="note">{live ? `${devices.length} ${devices.length === 1 ? 'device' : 'devices'} through OpenRGB. Click a part to see what it is.` : 'OpenRGB is not connected right now; the parts are what it reported last.'}</p>
		{:else if !held.length}
			<p class="note">Nothing from OpenRGB yet.</p>
		{/if}
		{#if held.length}
			<section class="left" aria-labelledby="held-title">
				<h2 id="held-title" class="section-title">Run by other software</h2>
				<ul>
					{#each held as h, i (i)}
						<li>
							<span class="two">{h.name}<span class="n">{h.by} has it</span></span>
							<button type="button" class="btn-quiet" aria-label={`Light ${h.name} with uncoil, even while ${h.by} runs`} onclick={() => choose(h.name, 'uncoil')}>Light it with uncoil</button>
						</li>
					{/each}
				</ul>
				<p class="note">uncoil leaves these alone while that program runs, and lights them again a few seconds after it quits.</p>
			</section>
		{/if}
		{#if pins.length}
			<section class="left" aria-labelledby="pins-title">
				<h2 id="pins-title" class="section-title">Your choices</h2>
				<ul>
					{#each pins as p (p.match)}
						<li>
							<span class="two">{p.match}<span class="n">{whoLabel(p.to)}</span></span>
							<button type="button" class="btn-quiet" aria-label={`Let uncoil decide who lights ${p.match}`} onclick={() => choose(p.match, 'auto')}>Automatic</button>
						</li>
					{/each}
				</ul>
			</section>
		{/if}
		{#if excluded.length}
			<section class="left" aria-labelledby="left-title">
				<h2 id="left-title" class="section-title">Left to their own software</h2>
				<ul>
					{#each excluded as name (name)}
						<li>
							<span>{name}</span>
							<button type="button" class="btn-quiet" aria-label={`Light ${name} with uncoil again`} onclick={() => setExcluded(excluded.filter((e) => e !== name))}>Light it again</button>
						</li>
					{/each}
				</ul>
			</section>
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
	.leave,
	.left,
	.who {
		display: grid;
		gap: 6px;
	}
	.who {
		padding-top: 10px;
		border-top: var(--hair);
	}
	.who .section-title {
		margin: 0;
	}
	.two {
		display: grid;
		gap: 2px;
		min-width: 0;
	}
	.leave {
		justify-items: start;
		padding-top: 10px;
		border-top: var(--hair);
	}
	.left {
		margin-top: 16px;
	}
	.left .section-title {
		margin: 0;
	}
	.left li {
		align-items: center;
	}
	.n {
		color: var(--color-ink-3);
		font-variant-numeric: tabular-nums;
	}
</style>
