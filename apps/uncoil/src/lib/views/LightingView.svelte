<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { Layers, Paintbrush, Pause, Play, Undo2 } from '@lucide/svelte';
	import { ms } from '#lib/motion.ts';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import DeskPreview, { type Hit } from '#lib/components/DeskPreview.svelte';
	import EffectSettings from '#lib/components/EffectSettings.svelte';
	import EffectSwatch from '#lib/components/EffectSwatch.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { EFFECTS, effectInfo, swatchFor, type LayerKind } from '#lib/effects.ts';
	import { coverage, hasLight, lightsOf, markedOf, paint } from '#lib/masks.ts';
	import { createPreview } from '#lib/preview.svelte.ts';
	import { shortName } from '#lib/daemon.svelte.ts';
	import { app } from '#lib/state.svelte.ts';
	import { baseOf, dropZone, editFor, effectOn, tidy, zonable, zoneFor, type Target } from '#lib/zones.svelte.ts';
	import type { Config, LayerEffect, Mask } from '#lib/types.ts';

	let { config, onstudio }: { config: Config; onstudio: () => void } = $props();

	const preview = createPreview(() => config);
	onMount(() => preview.start());

	// ---- what is being lit: the whole desk, some devices, or picked lights ------------------------------
	let target = $state<Target>({ kind: 'desk' });
	const zoned = $derived(zonable(config));
	const PC = 'openrgb:';
	/** One chip per device, OpenRGB's PC devices together as one. */
	const chips = $derived.by(() => {
		const out: { key: string; label: string; ids: string[] }[] = [];
		const pcIds: string[] = [];
		for (const d of preview.state.desk) {
			if (d.id.startsWith(PC)) pcIds.push(d.id);
			else out.push({ key: d.id, label: shortName(d.name), ids: [d.id] });
		}
		if (pcIds.length) out.push({ key: PC, label: 'PC', ids: pcIds });
		return out;
	});
	const name = (id: string) => {
		if (id.startsWith(PC)) return 'PC';
		const d = preview.state.desk.find((x) => x.id === id);
		return d ? shortName(d.name) : id;
	};
	const chosenIds = $derived(target.kind === 'devices' ? new Set(target.ids) : new Set<string>());
	const chipOn = (ids: string[]) => ids.every((id) => chosenIds.has(id));

	function choose(t: Target) {
		const keep = zoneFor(config, t);
		target = t;
		tidy(config, keep);
		if (t.kind !== 'desk') editFor(config, t, name);
	}
	/** A device chip or a click on the desk: choose it alone, or add / remove it with Ctrl or Shift. */
	function toggle(ids: string[], add: boolean) {
		const cur = target.kind === 'devices' ? target.ids : [];
		const on = ids.every((id) => cur.includes(id));
		let next: string[];
		if (add) next = on ? cur.filter((id) => !ids.includes(id)) : [...cur, ...ids.filter((id) => !cur.includes(id))];
		else next = on && cur.length === ids.length ? [] : [...ids];
		choose(next.length ? { kind: 'devices', ids: next } : { kind: 'desk' });
	}
	function clickDevice(id: string, add: boolean) {
		const chip = chips.find((c) => c.ids.includes(id));
		if (chip) toggle(chip.ids, add);
	}
	onDestroy(() => tidy(config));

	const hasAny = (l: { mask: Mask }) => lightsOf(l.mask).length > 0;

	// ---- the effect being edited -------------------------------------------------------------------
	const zone = $derived(zoned ? zoneFor(config, target) : undefined);
	const current = $derived<LayerEffect | null>(!zoned ? null : target.kind === 'desk' ? baseOf(config) : (zone?.effect ?? null));

	// Remember each effect's settings while you flip between them.
	const remembered = new Map<LayerKind, LayerEffect>();
	function setKind(kind: LayerKind) {
		if (!zoned) {
			// a Studio composition: picking an effect replaces it
			config.effect = structuredClone(remembered.get(kind) ?? effectInfo(kind)!.make());
			target = { kind: 'desk' };
			return;
		}
		const ed = editFor(config, target, name);
		const was = ed.get();
		if (was.kind === kind) return;
		remembered.set(was.kind, $state.snapshot(was) as LayerEffect);
		ed.set(structuredClone(remembered.get(kind) ?? effectInfo(kind)!.make()));
	}

	// Picking lights: click or drag across any devices; the first light decides add or remove.
	let paintAdds = true;
	function pick(h: Hit, phase: 'start' | 'move') {
		const z = zoneFor(config, { kind: 'lights' });
		if (!z) return;
		if (phase === 'start') paintAdds = !hasLight(z.mask, [h.device, h.shape]);
		z.mask = paint(z.mask, [h.device, h.shape], paintAdds);
	}
	const marked = $derived(target.kind === 'lights' && zone ? markedOf(zone.mask) : undefined);

	const liveIds = $derived(new Set(app.status?.devices.map((d) => d.id) ?? []));
	const away = $derived(new Set(app.status ? preview.state.desk.filter((d) => !liveIds.has(d.id) && !d.id.startsWith(PC)).map((d) => d.id) : []));
	const info = $derived(current ? effectInfo(current.kind) : null);
	const needsKeys = $derived(preview.uses(['reactive', 'ripple']));
	const where = $derived(
		target.kind === 'desk'
			? 'On the whole desk'
			: target.kind === 'lights'
				? zone && hasAny(zone)
					? coverage(zone.mask, name).replace(/^./, (c) => c.toUpperCase())
					: 'No lights picked yet: click or drag on the desk'
				: `On ${[...new Set(target.ids.map(name))].join(', ')}`
	);
	const pct = (v: number) => `${Math.round(v)}%`;
</script>

<Workspace title={pageTitle('lighting')} subtitle="One effect across your desk, or a different one per device. Changes apply as you make them." panelLabel="Effect settings">
	{#snippet tools()}
		{#if target.kind === 'lights'}
			<span class="hint">Click or drag on the desk to pick lights</span>
		{/if}
		<button class="btn-quiet" type="button" aria-pressed={preview.state.paused} onclick={() => (preview.state.paused = !preview.state.paused)}>
			{#if preview.state.paused}<Play size={14} />Play preview{:else}<Pause size={14} />Pause preview{/if}
		</button>
	{/snippet}

	<div class="stage-desk">
		<DeskPreview
			desk={preview.state.desk}
			colors={preview.state.colors}
			{away}
			{marked}
			chosen={chosenIds}
			onpick={target.kind === 'lights' ? pick : undefined}
			ondevice={zoned && target.kind !== 'lights' ? clickDevice : undefined}
			pickLabel={target.kind === 'lights' ? 'Your desk. Click or drag to pick lights.' : 'Your desk. Click a device to give it its own effect; Ctrl-click to add another.'}
		/>
	</div>

	{#if zoned}
		<div class="zones" role="group" aria-label="What the effect lights">
			<button type="button" class="chip" aria-pressed={target.kind === 'desk'} onclick={() => choose({ kind: 'desk' })}>
				<span class="dot" style:background={swatchFor(baseOf(config))}></span>Whole desk
			</button>
			{#each chips as c (c.key)}
				{@const own = c.ids.some((id) => effectOn(config, id) !== baseOf(config))}
				<button type="button" class="chip" aria-pressed={chipOn(c.ids)} onclick={(e) => toggle(c.ids, e.ctrlKey || e.shiftKey || e.metaKey)} title={own ? `${c.label} has its own effect` : `${c.label} follows the desk`}>
					<span class="dot" class:follows={!own} style:background={swatchFor(effectOn(config, c.ids[0]))}></span>{c.label}
				</button>
			{/each}
			<button type="button" class="chip" aria-pressed={target.kind === 'lights'} onclick={() => choose(target.kind === 'lights' ? { kind: 'desk' } : { kind: 'lights' })}>
				<Paintbrush size={13} />Pick lights
			</button>
			<span class="hint zones-hint">Or click a device on the desk; Ctrl-click to choose several</span>
		</div>
	{/if}

	<section aria-labelledby="effect-title">
		<h2 id="effect-title" class="section-title">Effect</h2>
		<div class="cards" role="radiogroup" aria-labelledby="effect-title">
			{#each EFFECTS as e (e.kind)}
				{@const shown = current?.kind === e.kind ? current : (remembered.get(e.kind) ?? e.make())}
				<button type="button" class="card" role="radio" aria-checked={current?.kind === e.kind} title={e.note} onclick={() => setKind(e.kind)}>
					<span class="swatch"><EffectSwatch effect={shown} /></span>
					<span class="name">{e.label}</span>
					<span class="note">{e.note}</span>
				</button>
			{/each}
			<button type="button" class="card studio" role="radio" aria-checked={!zoned} onclick={onstudio}>
				<span class="swatch studio-swatch"><Layers size={22} strokeWidth={1.6} /></span>
				<span class="name">Studio</span>
				<span class="note">Layer effects and paint lights yourself</span>
			</button>
		</div>
	</section>

	{#snippet panel()}
		{#key `${current?.kind ?? 'studio'}/${target.kind}`}
			<div class="settings" in:fade={{ duration: ms(160) }}>
				{#if !zoned || !current}
					<h2 class="section-title">Studio</h2>
					<p class="note">Your desk is running a layered Studio composition{config.effect.kind === 'studio' ? ` with ${config.effect.layers.length} ${config.effect.layers.length === 1 ? 'layer' : 'layers'}` : ''}. Pick an effect on the left to replace it, or keep editing it.</p>
					<button type="button" class="btn" onclick={onstudio}><Layers size={14} />Open Studio</button>
				{:else}
					<div class="head">
						<h2 class="section-title">{info?.label}</h2>
						<p class="where">{where}</p>
					</div>
					<EffectSettings effect={current} />
					{#if needsKeys}
						<Toggle label="Simulate typing in the preview" bind:checked={preview.state.simulateTyping} />
					{/if}
					{#if target.kind !== 'desk' && zone}
						<button type="button" class="btn-quiet" onclick={() => { dropZone(config, target); target = { kind: 'desk' }; }}>
							<Undo2 size={14} />{target.kind === 'lights' ? 'Drop the picked lights' : "Use the desk's effect"}
						</button>
					{/if}
				{/if}
				{#if config.effect.kind !== 'off'}
					<div class="divider"></div>
					<Slider label="Brightness" min={0} max={100} bind:value={() => Math.round(config.brightness * 100), (v) => (config.brightness = v / 100)} format={pct} />
					<Slider label="Saturation" min={0} max={100} bind:value={() => Math.round(config.saturation * 100), (v) => (config.saturation = v / 100)} format={pct} ends={['White', 'Vivid']} />
				{/if}
			</div>
		{/key}
	{/snippet}
</Workspace>

<style>
	/* The desk's own proportions (mat plus margin), so there is no empty band above or below it. */
	.stage-desk {
		width: 100%;
		aspect-ratio: 2.56;
		max-height: 40vh;
	}
	section {
		display: grid;
		gap: 10px;
	}
	.hint {
		color: var(--color-ink-3);
		font-size: 12px;
	}
	.zones {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
	}
	.zones-hint {
		margin-left: 6px;
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		height: 30px;
		padding: 0 12px 0 9px;
		border: 1px solid var(--color-seam-2);
		border-radius: 999px;
		background: var(--color-surface);
		font-weight: 500;
		transition:
			background-color var(--t-mid) var(--ease),
			border-color var(--t-mid) var(--ease);
	}
	.chip:hover {
		background: var(--color-surface-2);
	}
	.chip[aria-pressed='true'] {
		border-color: var(--color-select);
		box-shadow: inset 0 0 0 1px var(--color-select);
	}
	/* a gel chip: the effect this device shows; hollow when it just follows the desk */
	.dot {
		width: 10px;
		height: 14px;
		border-radius: 2px;
		box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.15);
	}
	.dot.follows {
		opacity: 0.45;
	}
	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(128px, 1fr));
		gap: 8px;
	}
	.card {
		display: grid;
		align-content: start;
		gap: 2px;
		padding: 6px 6px 9px;
		border: 1px solid var(--color-seam);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		text-align: left;
		transition:
			border-color var(--t-mid) var(--ease),
			box-shadow var(--t-mid) var(--ease);
	}
	.card:hover {
		border-color: var(--color-seam-2);
	}
	.card[aria-checked='true'] {
		border-color: var(--color-select);
		box-shadow: inset 0 0 0 1px var(--color-select);
	}
	.swatch {
		display: block;
		height: 44px;
		margin-bottom: 5px;
		border-radius: var(--radius-sm);
	}
	.studio-swatch {
		display: grid;
		place-items: center;
		background: var(--color-surface-2);
		color: var(--color-ink-2);
	}
	.name {
		padding: 0 3px;
		font-weight: 600;
	}
	.note {
		padding: 0 3px;
		color: var(--color-ink-3);
		font-size: 11.5px;
		line-height: 1.35;
	}
	.settings {
		display: grid;
		gap: 18px;
	}
	.settings > .note {
		padding: 0;
		font-size: 12px;
	}
	.head {
		display: grid;
		gap: 2px;
	}
	.where {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
	}
	.divider {
		height: 1px;
		background: var(--color-seam);
	}
</style>
