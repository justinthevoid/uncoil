<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { Layers, Pause, Play } from '@lucide/svelte';
	import { ms } from '#lib/motion.ts';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import DeskPreview from '#lib/components/DeskPreview.svelte';
	import EffectSettings from '#lib/components/EffectSettings.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { EFFECTS, effectInfo, swatchFor, type LayerKind } from '#lib/effects.ts';
	import { createPreview } from '#lib/preview.svelte.ts';
	import { app } from '#lib/state.svelte.ts';
	import type { Config, LayerEffect } from '#lib/types.ts';

	let { config, onstudio }: { config: Config; onstudio: () => void } = $props();

	const preview = createPreview(() => config);
	onMount(() => preview.start());

	// Remember each effect's settings while you flip between them.
	const remembered = new Map<LayerKind, LayerEffect>();
	function setKind(kind: LayerKind) {
		if (config.effect.kind === kind) return;
		if (config.effect.kind !== 'studio') remembered.set(config.effect.kind, $state.snapshot(config.effect) as LayerEffect);
		config.effect = structuredClone(remembered.get(kind) ?? effectInfo(kind)!.make());
	}

	const liveIds = $derived(new Set(app.status?.devices.map((d) => d.id) ?? []));
	const away = $derived(new Set(app.status ? preview.state.desk.filter((d) => !liveIds.has(d.id)).map((d) => d.id) : []));
	const info = $derived(config.effect.kind === 'studio' ? null : effectInfo(config.effect.kind));
	const needsKeys = $derived(preview.uses(['reactive', 'ripple']));
	const pct = (v: number) => `${Math.round(v)}%`;
</script>

<Workspace title={pageTitle('lighting')} subtitle="One effect across your whole desk. Changes apply as you make them." panelLabel="Effect settings">
	{#snippet tools()}
		{#if needsKeys}
			<span class="hint">Click the desk to press a key</span>
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
			onpick={needsKeys ? (h, phase) => phase === 'start' && preview.press(h.x, h.y) : undefined}
			pickLabel="Your desk. Click a key to preview a key press."
		/>
	</div>

	<section aria-labelledby="effect-title">
		<h2 id="effect-title" class="section-title">Effect</h2>
		<div class="cards" role="radiogroup" aria-labelledby="effect-title">
			{#each EFFECTS as e (e.kind)}
				{@const shown = config.effect.kind === e.kind ? (config.effect as LayerEffect) : (remembered.get(e.kind) ?? e.make())}
				<button type="button" class="card" role="radio" aria-checked={config.effect.kind === e.kind} title={e.note} onclick={() => setKind(e.kind)}>
					<span class="swatch" style:background={swatchFor(shown)}></span>
					<span class="name">{e.label}</span>
					<span class="note">{e.note}</span>
				</button>
			{/each}
			<button type="button" class="card studio" role="radio" aria-checked={config.effect.kind === 'studio'} onclick={onstudio}>
				<span class="swatch studio-swatch"><Layers size={22} strokeWidth={1.6} /></span>
				<span class="name">Studio</span>
				<span class="note">Layer effects and paint keys yourself</span>
			</button>
		</div>
	</section>

	{#snippet panel()}
		{#key config.effect.kind}
			<div class="settings" in:fade={{ duration: ms(160) }}>
				{#if config.effect.kind === 'studio'}
					<h2 class="section-title">Studio</h2>
					<p class="note">Your desk is running a layered Studio composition with {config.effect.layers.length} {config.effect.layers.length === 1 ? 'layer' : 'layers'}. Pick an effect on the left to replace it, or keep editing it.</p>
					<button type="button" class="btn" onclick={onstudio}><Layers size={14} />Open Studio</button>
				{:else}
					<h2 class="section-title">{info?.label}</h2>
					<EffectSettings effect={config.effect} />
					{#if needsKeys}
						<Toggle label="Simulate typing in the preview" bind:checked={preview.state.simulateTyping} />
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
		height: 40px;
		margin-bottom: 5px;
		border-radius: var(--radius-sm);
		box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.08);
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
	.divider {
		height: 1px;
		background: var(--color-seam);
	}
</style>
