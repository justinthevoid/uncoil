<script lang="ts">
	import { onMount } from 'svelte';
	import { fly, fade } from 'svelte/transition';
	import { expoOut } from 'svelte/easing';
	import { Pause, Play } from '@lucide/svelte';
	import { ms, reducedMotion } from '#lib/motion.ts';
	import DeskPreview from '#lib/components/DeskPreview.svelte';
	import Dial from '#lib/components/Dial.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { getDesk, previewFrame } from '#lib/api.ts';
	import { app } from '#lib/state.svelte.ts';
	import type { Config, DeskDevice, Effect, EffectKind, Rgb } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	const kinds: { value: EffectKind; label: string }[] = [
		{ value: 'wave', label: 'Wave' },
		{ value: 'spectrum', label: 'Spectrum' },
		{ value: 'static', label: 'Static' },
		{ value: 'off', label: 'Off' }
	];

	// Remember each effect's settings while you flip between them; seeded with the engine's defaults.
	const remembered: Record<EffectKind, Effect> = {
		wave: { kind: 'wave', angle_deg: 35, period_s: 14, wavelength: 26, reverse: false },
		spectrum: { kind: 'spectrum', period_s: 14 },
		static: { kind: 'static', color: [226, 55, 44] },
		off: { kind: 'off' }
	};
	function setKind(kind: EffectKind) {
		if (kind === config.effect.kind) return;
		remembered[config.effect.kind] = $state.snapshot(config.effect) as Effect;
		const next = structuredClone(remembered[kind]);
		if ('period_s' in next && 'period_s' in config.effect) next.period_s = config.effect.period_s;
		config.effect = next;
	}

	// Speed on a log scale: reads evenly from a slow drift (60 s per cycle) to a quick cycle (2 s).
	const SLOW = 60;
	const FAST = 2;
	const toPos = (period: number) => (100 * Math.log(period / SLOW)) / Math.log(FAST / SLOW);
	const toPeriod = (pos: number) => Math.round(SLOW * Math.pow(FAST / SLOW, pos / 100) * 10) / 10;
	const fmtPeriod = (p: number) => `${p < 10 ? p.toFixed(1) : Math.round(p)} s`;
	const pct = (v: number) => `${Math.round(v)}%`;
	const toHex = ([r, g, b]: Rgb) => '#' + [r, g, b].map((c) => c.toString(16).padStart(2, '0')).join('');
	const fromHex = (h: string): Rgb => {
		const n = parseInt(h.slice(1), 16);
		return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
	};
	const colorId = $props.id();

	// One effect clock for the preview. Reduced motion starts paused.
	let paused = $state(reducedMotion());
	let clock = 6;
	let lastNow = performance.now();
	const time = () => {
		const now = performance.now();
		if (!paused) clock += (now - lastNow) / 1000;
		lastNow = now;
		return clock;
	};

	// Desk geometry once; live LED colours from the real engine code, ~25 times a second.
	let desk = $state<DeskDevice[]>([]);
	let colors = $state<string[][]>([]);
	onMount(() => {
		let alive = true;
		let busy = false;
		getDesk($state.snapshot(config) as Config).then(async (d) => {
			if (!alive) return;
			desk = d;
			colors = await previewFrame($state.snapshot(config) as Config, time());
		});
		const id = setInterval(async () => {
			if (!desk.length || busy || (document.hidden && colors.length)) return;
			busy = true;
			try {
				const f = await previewFrame($state.snapshot(config) as Config, time());
				if (alive) colors = f;
			} finally {
				busy = false;
			}
		}, 40);
		return () => {
			alive = false;
			clearInterval(id);
		};
	});

	const liveIds = $derived(new Set(app.status?.devices.map((d) => d.id) ?? []));
	const engineUp = $derived(!!app.status);
	const away = $derived(new Set(engineUp ? desk.filter((d) => !liveIds.has(d.id)).map((d) => d.id) : []));
</script>

<section class="lighting" aria-labelledby="lighting-title">
	<header class="head">
		<div>
			<h1 id="lighting-title" class="page-title">Lighting</h1>
			<p class="lede">One effect across your whole desk. Changes apply as you make them.</p>
		</div>
		<button class="btn-quiet" type="button" aria-pressed={paused} onclick={() => (paused = !paused)}>
			{#if paused}<Play size={14} />Play preview{:else}<Pause size={14} />Pause preview{/if}
		</button>
	</header>

	<div class="stage">
		<DeskPreview {desk} {colors} {away} />
	</div>

	<ul class="chips" aria-label="Devices">
		{#each desk as d (d.id)}
			{@const state = !engineUp ? 'Preview only' : liveIds.has(d.id) ? 'Live' : 'Not connected'}
			<li class:live={engineUp && liveIds.has(d.id)} class:off={engineUp && !liveIds.has(d.id)}>
				<span class="lamp" aria-hidden="true"></span>{d.name.replace(/^Razer /, '')}<span class="state">{state}</span>
			</li>
		{/each}
	</ul>

	<div class="controls">
		<div class="effect">
			<Segmented label="Effect" options={kinds} value={config.effect.kind} onchange={setKind} />
			{#key config.effect.kind}
				<div class="params" in:fly={{ y: 8, duration: ms(320), easing: expoOut }} out:fade={{ duration: ms(100) }}>
					{#if config.effect.kind === 'wave'}
						{@const wave = config.effect}
						<Dial label="Direction" bind:value={wave.angle_deg} />
						<div class="col">
							<Slider label="Speed" min={0} max={100} step={0.5} bind:value={() => toPos(wave.period_s), (v) => (wave.period_s = toPeriod(v))} format={(v) => `${fmtPeriod(toPeriod(v))} per cycle`} ends={['Slower', 'Faster']} />
							<Slider label="Band width" min={6} max={60} bind:value={wave.wavelength} format={(v) => `${v} keys`} ends={['Tight', 'Broad']} />
							<Toggle label="Reverse direction" bind:checked={wave.reverse} />
						</div>
					{:else if config.effect.kind === 'spectrum'}
						{@const spectrum = config.effect}
						<div class="col wide">
							<Slider label="Speed" min={0} max={100} step={0.5} bind:value={() => toPos(spectrum.period_s), (v) => (spectrum.period_s = toPeriod(v))} format={(v) => `${fmtPeriod(toPeriod(v))} per cycle`} ends={['Slower', 'Faster']} hint="Every LED shows the same colour and moves through the rainbow together." />
						</div>
					{:else if config.effect.kind === 'static'}
						{@const stat = config.effect}
						<div class="col wide">
							<label class="label" for={colorId}>Colour</label>
							<div class="swatch-row">
								<input id={colorId} type="color" value={toHex(stat.color)} oninput={(e) => (stat.color = fromHex(e.currentTarget.value))} />
								<output class="num" for={colorId}>{toHex(stat.color).toUpperCase()}</output>
							</div>
						</div>
					{:else}
						<p class="off-note">Lighting is off. Your devices stay dark until you choose another effect.</p>
					{/if}
				</div>
			{/key}
		</div>
		<div class="output">
			<Slider label="Brightness" min={0} max={100} bind:value={() => Math.round(config.brightness * 100), (v) => (config.brightness = v / 100)} format={pct} disabled={config.effect.kind === 'off'} />
			<Slider label="Saturation" min={0} max={100} bind:value={() => Math.round(config.saturation * 100), (v) => (config.saturation = v / 100)} format={pct} ends={['White', 'Vivid']} disabled={config.effect.kind === 'off' || config.effect.kind === 'static'} />
		</div>
	</div>
</section>

<style>
	.lighting {
		display: grid;
		grid-template-rows: auto minmax(180px, 1fr) auto auto;
		gap: 14px;
		height: 100%;
		min-height: 0;
	}
	.head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 24px;
	}
	.lede {
		margin: 6px 0 0;
		color: var(--color-ink-3);
		font-size: 13px;
	}
	.stage {
		min-height: 0;
		padding: 18px;
		border-radius: var(--radius-lg);
		background: radial-gradient(ellipse at 50% 40%, #151515, #0d0d0d 70%);
		border: 1px solid var(--color-seam);
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.chips li {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 30px;
		padding: 0 12px;
		border-radius: 15px;
		background: var(--color-surface);
		font-size: 12px;
		font-weight: 500;
	}
	.state {
		color: var(--color-ink-3);
		font-weight: 400;
	}
	.lamp {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--color-ink-4);
	}
	.chips li.live .lamp {
		background: #3fb950;
	}
	.chips li.off {
		color: var(--color-ink-3);
	}
	.controls {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(220px, 0.45fr);
		gap: 28px;
		padding: 18px 20px;
		border-radius: var(--radius-lg);
		background: var(--color-raised);
		border: 1px solid var(--color-seam);
	}
	.effect {
		display: grid;
		gap: 18px;
		align-content: start;
		min-width: 0;
	}
	.params {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 28px;
		align-items: start;
	}
	.col {
		display: grid;
		gap: 14px;
		min-width: 0;
	}
	.col.wide {
		grid-column: 1 / -1;
		gap: 8px;
	}
	.output {
		display: grid;
		gap: 18px;
		align-content: start;
		padding-left: 28px;
		border-left: 1px solid var(--color-seam);
	}
	.off-note {
		grid-column: 1 / -1;
		margin: 0;
		color: var(--color-ink-3);
		font-size: 13px;
	}
	.swatch-row {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.swatch-row output {
		font-size: 13px;
		color: var(--color-ink-2);
	}
	input[type='color'] {
		appearance: none;
		width: 56px;
		height: 32px;
		padding: 0;
		border: 1px solid var(--color-seam-2);
		border-radius: var(--radius);
		background: none;
		overflow: hidden;
	}
	input[type='color']::-webkit-color-swatch-wrapper {
		padding: 3px;
	}
	input[type='color']::-webkit-color-swatch {
		border: 0;
		border-radius: var(--radius-sm);
	}
	@container view (max-width: 820px) {
		.lighting {
			grid-template-rows: auto 240px auto auto;
			height: auto;
		}
		.controls {
			grid-template-columns: minmax(0, 1fr);
		}
		.output {
			padding-left: 0;
			border-left: 0;
		}
	}
</style>
