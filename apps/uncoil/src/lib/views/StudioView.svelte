<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { ArrowDown, ArrowUp, Eye, EyeOff, Pause, Play, Plus, Trash2, Paintbrush } from '@lucide/svelte';
	import { ms } from '#lib/motion.ts';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import { shortName } from '#lib/daemon.svelte.ts';
	import DeskPreview, { type Hit } from '#lib/components/DeskPreview.svelte';
	import EffectSettings from '#lib/components/EffectSettings.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import { EFFECTS, effectInfo, swatchFor, type LayerKind } from '#lib/effects.ts';
	import { createPreview } from '#lib/preview.svelte.ts';
	import { allLightsOf, coverage as coverageOf, hasLight, lightsOf, markedOf, paint } from '#lib/masks.ts';
	import type { Config, LayerEffect, Mask, StudioEffect, StudioLayer } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	const preview = createPreview(() => config);
	onMount(() => preview.start());

	const studio = $derived(config.effect.kind === 'studio' ? (config.effect as StudioEffect) : null);
	let selected = $state(0);
	const layer = $derived(studio?.layers[selected] ?? null);

	function begin(fromCurrent: boolean) {
		const base: LayerEffect = fromCurrent && config.effect.kind !== 'studio' ? ($state.snapshot(config.effect) as LayerEffect) : { kind: 'off' };
		config.effect = { kind: 'studio', layers: [{ name: fromCurrent ? (effectInfo(base.kind)?.label ?? 'Base') : 'Base', enabled: true, opacity: 1, effect: base, mask: { kind: 'all' } }] };
		selected = 0;
	}

	let adding = $state(false);
	function addLayer(kind: LayerKind, painted = false) {
		if (!studio) return;
		const l: StudioLayer = {
			name: painted ? 'Painted lights' : (effectInfo(kind)?.label ?? 'Layer'),
			enabled: true,
			opacity: 1,
			effect: effectInfo(kind)!.make(),
			mask: painted ? { kind: 'lights', lights: [] } : { kind: 'all' }
		};
		studio.layers.push(l);
		selected = studio.layers.length - 1;
		adding = false;
	}
	function move(i: number, d: -1 | 1) {
		if (!studio) return;
		const j = i + d;
		if (j < 0 || j >= studio.layers.length) return;
		const [l] = studio.layers.splice(i, 1);
		studio.layers.splice(j, 0, l);
		selected = j;
	}
	function remove(i: number) {
		if (!studio || studio.layers.length <= 1) return;
		studio.layers.splice(i, 1);
		selected = Math.max(0, Math.min(selected, studio.layers.length - 1));
	}

	// ---- masks -------------------------------------------------------------------------------------
	const deviceName = (id: string) => { const n = preview.state.desk.find((d) => d.id === id)?.name; return n ? shortName(n) : id; };
	const coverage = (m: Mask) => coverageOf(m, deviceName);
	/** The Covers choice: older single-device key masks show as Lights. */
	type Covers = 'all' | 'devices' | 'lights';
	const picking = (m: Mask | undefined) => m?.kind === 'keys' || m?.kind === 'lights';
	const coversOf = (m: Mask): Covers => (picking(m) ? 'lights' : (m.kind as Covers));
	function setMaskKind(k: Covers) {
		if (!layer || coversOf(layer.mask) === k) return;
		const kb = preview.state.desk.find((d) => d.kind === 'keyboard') ?? preview.state.desk[0];
		layer.mask = k === 'all' ? { kind: 'all' } : k === 'devices' ? { kind: 'devices', ids: kb ? [kb.id] : [] } : { kind: 'lights', lights: [] };
	}
	function toggleDevice(id: string) {
		if (layer?.mask.kind !== 'devices') return;
		const ids = layer.mask.ids;
		layer.mask.ids = ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id];
	}
	// Click or drag on the desk, across devices: the first light decides whether the drag adds or removes.
	let paintAdds = true;
	function pick(h: Hit, phase: 'start' | 'move') {
		if (!layer || !picking(layer.mask)) return;
		if (phase === 'start') paintAdds = !hasLight(layer.mask, [h.device, h.shape]);
		layer.mask = paint(layer.mask, [h.device, h.shape], paintAdds);
	}
	const marked = $derived(layer && picking(layer.mask) ? markedOf(layer.mask) : undefined);
	/** Add every light of one device to the picked lights. */
	function selectDevice(id: string) {
		if (!layer || !picking(layer.mask)) return;
		const have = lightsOf(layer.mask);
		const add = allLightsOf(preview.state.desk, [id]).filter(([d, sh]) => !have.some(([d2, s2]) => d === d2 && sh === s2));
		layer.mask = { kind: 'lights', lights: [...have, ...add] };
	}

	function setEffectKind(kind: LayerKind) {
		if (!layer || layer.effect.kind === kind) return;
		const wasDefaultName = layer.name === effectInfo(layer.effect.kind)?.label;
		layer.effect = effectInfo(kind)!.make();
		if (wasDefaultName) layer.name = effectInfo(kind)!.label;
	}
	const displayOrder = $derived(studio ? studio.layers.map((l, i) => ({ l, i })).reverse() : []);
</script>

<Workspace title={pageTitle('studio')} subtitle="Stack effects in layers. Each layer can cover the whole desk, some devices, or lights you pick." panelLabel="Layer settings">
	{#snippet tools()}
		{#if picking(layer?.mask)}<span class="hint">Click or drag on the desk to pick lights</span>{/if}
		<button class="btn-quiet" type="button" aria-pressed={preview.state.paused} onclick={() => (preview.state.paused = !preview.state.paused)}>
			{#if preview.state.paused}<Play size={14} />Play preview{:else}<Pause size={14} />Pause preview{/if}
		</button>
	{/snippet}

	<div class="stage-desk">
		<DeskPreview
			desk={preview.state.desk}
			colors={preview.state.colors}
			{marked}
			onpick={picking(layer?.mask) ? pick : preview.needs('keys') ? (h, phase) => phase === 'start' && preview.press(h.x, h.y) : undefined}
			pickLabel={picking(layer?.mask) ? 'Your desk. Click or drag to pick lights for this layer.' : 'Your desk. Click a key to preview a key press.'}
		/>
	</div>

	{#if !studio}
		<div class="empty" in:fade={{ duration: ms(160) }}>
			<h2 class="section-title">Make your own lighting</h2>
			<p>Studio stacks effects on top of each other. Start with your current effect as the bottom layer, then add layers on top: a static colour on WASD, a ripple over the wave, the mat in one colour.</p>
			<div class="row">
				<button type="button" class="btn" onclick={() => begin(true)}>Start from the current effect</button>
				<button type="button" class="btn-quiet" onclick={() => begin(false)}>Start blank</button>
			</div>
		</div>
	{:else}
		<section class="layers" aria-labelledby="layers-title">
			<div class="layers-head">
				<h2 id="layers-title" class="section-title">Layers <span class="count">top to bottom</span></h2>
				<div class="row">
					<button type="button" class="btn-quiet" onclick={() => addLayer('static', true)}><Paintbrush size={14} />Paint lights</button>
					<button type="button" class="btn-quiet" aria-expanded={adding} onclick={() => (adding = !adding)}><Plus size={14} />Add layer</button>
				</div>
			</div>
			{#if adding}
				<div class="add" in:fade={{ duration: ms(140) }}>
					{#each EFFECTS as e (e.kind)}
						<button type="button" class="add-card" onclick={() => addLayer(e.kind)}>
							<span class="mini" style:background={swatchFor(e.make())}></span>{e.label}
						</button>
					{/each}
				</div>
			{/if}
			<ul>
				{#each displayOrder as { l, i } (i)}
					<li class:sel={i === selected} class:off={!l.enabled}>
						<button type="button" class="pick" aria-pressed={i === selected} onclick={() => (selected = i)}>
							<span class="mini" style:background={swatchFor(l.effect)}></span>
							<span class="lname">{l.name}</span>
							<span class="lcov">{coverage(l.mask)}{l.opacity < 1 ? ` · ${Math.round(l.opacity * 100)}%` : ''}</span>
						</button>
						<button type="button" class="ib" aria-label={l.enabled ? `Hide ${l.name}` : `Show ${l.name}`} onclick={() => (l.enabled = !l.enabled)}>
							{#if l.enabled}<Eye size={15} />{:else}<EyeOff size={15} />{/if}
						</button>
						<button type="button" class="ib" aria-label="Move {l.name} up" disabled={i === studio.layers.length - 1} onclick={() => move(i, 1)}><ArrowUp size={15} /></button>
						<button type="button" class="ib" aria-label="Move {l.name} down" disabled={i === 0} onclick={() => move(i, -1)}><ArrowDown size={15} /></button>
						<button type="button" class="ib" aria-label="Delete {l.name}" disabled={studio.layers.length <= 1} onclick={() => remove(i)}><Trash2 size={15} /></button>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#snippet panel()}
		{#if layer}
			{#key selected}
				<div class="settings" in:fade={{ duration: ms(140) }}>
					<label class="field"><span class="label">Layer name</span><input class="input" bind:value={layer.name} /></label>

					<div class="field">
						<span class="label">Covers</span>
						<Segmented
							label="Covers"
							options={[
								{ value: 'all' as const, label: 'Desk' },
								{ value: 'devices' as const, label: 'Devices' },
								{ value: 'lights' as const, label: 'Lights' }
							]}
							value={coversOf(layer.mask)}
							onchange={setMaskKind}
						/>
						{#if layer.mask.kind === 'devices'}
							{@const ids = layer.mask.ids}
							<div class="devs">
								{#each preview.state.desk as d (d.id)}
									<label class="dev"><input type="checkbox" checked={ids.includes(d.id)} onchange={() => toggleDevice(d.id)} />{deviceName(d.id)}</label>
								{/each}
							</div>
						{:else if picking(layer.mask)}
							<p class="note">{coverage(layer.mask)}. Click or drag on the desk to add lights on any device; drag from a picked light to remove.</p>
							<div class="row">
								{#each preview.state.desk as d (d.id)}
									<button type="button" class="btn-quiet" onclick={() => selectDevice(d.id)}>All of {deviceName(d.id)}</button>
								{/each}
								<button type="button" class="btn-quiet" disabled={!lightsOf(layer.mask).length} onclick={() => layer && (layer.mask = { kind: 'lights', lights: [] })}>Clear</button>
							</div>
						{/if}
					</div>

					<Slider label="Opacity" min={0} max={100} bind:value={() => Math.round(layer.opacity * 100), (v) => (layer.opacity = v / 100)} format={(v) => `${Math.round(v)}%`} />

					<div class="divider"></div>
					<label class="field">
						<span class="label">Effect</span>
						<select class="input" value={layer.effect.kind} onchange={(e) => setEffectKind(e.currentTarget.value as LayerKind)}>
							{#each EFFECTS as e (e.kind)}<option value={e.kind}>{e.label}</option>{/each}
						</select>
					</label>
					<EffectSettings effect={layer.effect} />
				</div>
			{/key}
		{:else}
			<p class="note">Start a composition to edit layers.</p>
		{/if}
	{/snippet}
</Workspace>

<style>
	.stage-desk {
		width: 100%;
		aspect-ratio: 2.56;
		max-height: 40vh;
	}
	.hint {
		color: var(--color-ink-3);
		font-size: 12px;
	}
	.empty {
		display: grid;
		gap: 10px;
		justify-items: start;
		max-width: 620px;
	}
	.empty p {
		margin: 0;
		color: var(--color-ink-2);
		line-height: 1.55;
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.layers {
		display: grid;
		gap: 10px;
	}
	.layers-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
	}
	.count {
		margin-left: 6px;
		color: var(--color-ink-3);
		font-family: var(--font-sans);
		font-size: 12px;
		font-weight: 400;
	}
	.add {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
		gap: 6px;
		padding: 8px;
		border: var(--hair);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
	}
	.add-card {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 34px;
		padding: 0 8px 0 4px;
		border: 0;
		border-radius: var(--radius);
		background: none;
		font-weight: 500;
		text-align: left;
	}
	.add-card:hover {
		background: var(--color-surface-2);
	}
	.mini {
		width: 34px;
		height: 24px;
		border-radius: 4px;
		box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.1);
		flex: none;
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
		align-items: center;
		gap: 2px;
		padding-right: 6px;
		border: 1px solid var(--color-seam);
		border-radius: var(--radius);
		background: var(--color-surface);
		transition: opacity var(--t-mid) var(--ease);
	}
	li.sel {
		border-color: var(--color-select);
		box-shadow: inset 0 0 0 1px var(--color-select);
	}
	li.off .pick {
		opacity: 0.5;
	}
	.pick {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 10px;
		flex: 1;
		min-width: 0;
		height: 44px;
		padding: 0 10px 0 6px;
		border: 0;
		background: none;
		text-align: left;
	}
	.lname {
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.lcov {
		color: var(--color-ink-3);
		font-size: 12px;
		white-space: nowrap;
	}
	.ib {
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		border: 0;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--color-ink-3);
	}
	.ib:hover:not(:disabled) {
		background: var(--color-surface-2);
		color: var(--color-ink);
	}
	.ib:disabled {
		opacity: 0.3;
	}
	.settings {
		display: grid;
		gap: 18px;
	}
	.field {
		display: grid;
		gap: 8px;
	}
	.devs {
		display: grid;
		gap: 4px;
	}
	.dev {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.dev input {
		accent-color: var(--color-ink);
		margin: 0;
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
	}
	.divider {
		height: 1px;
		background: var(--color-seam);
	}
	select.input {
		appearance: auto;
	}
</style>
