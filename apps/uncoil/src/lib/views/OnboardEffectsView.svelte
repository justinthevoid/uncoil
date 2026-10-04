<script lang="ts">
	import { onMount } from 'svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import OptionList from '#lib/components/OptionList.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import PipeUnavailable from '#lib/components/PipeUnavailable.svelte';
	import { daemon } from '#lib/api.ts';
	import { pipe, loadDevices, errorText, experimentalBadge } from '#lib/daemon.svelte.ts';
	import type { Capabilities, EffectState } from '#lib/types.ts';

	let { deviceId }: { deviceId: string } = $props();

	onMount(() => {
		if (!pipe.loaded) loadDevices();
	});
	const device = $derived(pipe.devices.find((d) => d.id === deviceId) ?? null);
	const word = $derived(device?.kind === 'keyboard' ? 'keyboard' : device?.kind === 'mouse' ? 'mouse' : 'mat');

	let caps = $state<Capabilities | null>(null);
	$effect(() => {
		if (device) daemon<Capabilities>('capabilities', deviceId).then((c) => (caps = c)).catch(() => (caps = null));
	});

	const LABEL: Record<string, string> = { off: 'Off', static: 'Static', breathing: 'Breathing', spectrum: 'Spectrum', wave: 'Wave', wheel: 'Wheel', reactive: 'Reactive', starlight: 'Starlight' };
	let name = $state('spectrum');
	let color = $state('#e0a33e');
	let color2 = $state('#5b84e0');
	let two = $state(false);
	let dir = $state<'left' | 'right'>('left');
	let speed = $state(40);
	let busy = $state(false);
	let note = $state<{ ok: boolean; text: string } | null>(null);

	const spec = $derived.by(() => {
		switch (name) {
			case 'static':
			case 'reactive':
				return `${name} ${color}`;
			case 'breathing':
			case 'starlight':
				return two ? `${name} ${color} ${color2}` : `${name} ${color}`;
			case 'wave':
			case 'wheel':
				return `${name} ${dir} speed ${speed}`;
			default:
				return name;
		}
	});

	async function run(software: boolean) {
		busy = true;
		note = null;
		try {
			const r = software ? await daemon<EffectState>('effect.software', deviceId) : await daemon<EffectState>('effect.hw', deviceId, { effect: spec });
			pipe.devices = pipe.devices.map((d) => (d.id === deviceId ? { ...d, hw_effect: r.effect } : d));
			const fx = r.effect ? (LABEL[r.effect.split(' ')[0]] ?? r.effect).toLowerCase() : null;
			note = { ok: true, text: fx ? `The ${word} is running its built-in ${fx} effect.` : 'Back on uncoil’s desk-wide effect.' };
		} catch (e) {
			note = { ok: false, text: errorText(e) };
		} finally {
			busy = false;
		}
	}
</script>

{#if !pipe.loaded}
	<p class="loading">Connecting to the engine…</p>
{:else if pipe.unreachable || !device}
	<PipeUnavailable page="effects" unreachable={pipe.unreachable} />
{:else}
	<Workspace title={pageTitle('effects')} badge={experimentalBadge(deviceId)} subtitle="Let the {word} run one of its built-in effects by itself: no CPU at all, but it won't flow across the desk. Nothing is saved; unplugging or switching back ends it.">
		<div class="card">
			<div class="cols">
				<OptionList label="Effect" options={(caps?.hw_effects ?? []).map((e) => ({ value: e, label: LABEL[e] ?? e }))} value={name} onchange={(v) => (name = v)} />
				<div class="params">
					{#if name === 'static' || name === 'reactive' || name === 'breathing' || name === 'starlight'}
						<div class="colors">
							<label class="swatch"><span class="label">Colour</span><input type="color" bind:value={color} /></label>
							{#if name === 'breathing' || name === 'starlight'}
								<label class="swatch" class:off={!two}>
									<span class="label"><input type="checkbox" bind:checked={two} /> Second colour</span>
									<input type="color" bind:value={color2} disabled={!two} />
								</label>
							{/if}
						</div>
					{:else if name === 'wave' || name === 'wheel'}
						<Segmented label="Direction" options={[{ value: 'left' as const, label: 'Left' }, { value: 'right' as const, label: 'Right' }]} value={dir} onchange={(v) => (dir = v)} />
						<Slider label="Speed" min={10} max={120} bind:value={speed} ends={['Fast', 'Slow']} format={(v) => String(Math.round(v))} hint="40 is the device's default." />
					{:else}
						<p class="note">This effect has no settings.</p>
					{/if}
					<div class="row">
						<button type="button" class="btn" disabled={busy || !caps} onclick={() => run(false)}>Run on the {word}</button>
						{#if device.hw_effect}
							<button type="button" class="btn-quiet" disabled={busy} onclick={() => run(true)}>Back to uncoil's effect</button>
						{/if}
					</div>
					{#if note}<p class="outcome" class:bad={!note.ok} role="status">{note.text}</p>{/if}
				</div>
			</div>
		</div>
	</Workspace>
{/if}

<style>
	.loading {
		margin: 24px 28px;
		color: var(--color-ink-3);
	}
	.card {
		max-width: 820px;
		padding: 18px;
		border: var(--hair);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
	}
	.cols {
		display: grid;
		grid-template-columns: minmax(0, 0.8fr) minmax(0, 1fr);
		gap: 28px;
		align-items: start;
	}
	.params {
		display: grid;
		gap: 16px;
	}
	.colors {
		display: flex;
		gap: 18px;
	}
	.swatch {
		display: grid;
		gap: 8px;
		transition: opacity var(--t-mid) var(--ease);
	}
	.swatch.off {
		opacity: 0.55;
	}
	.swatch .label {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	input[type='checkbox'] {
		accent-color: var(--color-ink);
		margin: 0;
	}
	input[type='color'] {
		appearance: none;
		width: 56px;
		height: 32px;
		padding: 0;
		border: var(--hair-strong);
		border-radius: var(--radius-sm);
		background: none;
		overflow: hidden;
	}
	input[type='color']::-webkit-color-swatch-wrapper {
		padding: 2px;
	}
	input[type='color']::-webkit-color-swatch {
		border: 0;
		border-radius: 3px;
	}
	.row {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
	}
	@container view (max-width: 640px) {
		.cols {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
