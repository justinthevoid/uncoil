<script lang="ts">
	import { onMount } from 'svelte';
	import { fly, fade } from 'svelte/transition';
	import { expoOut } from 'svelte/easing';
	import { ms, reducedMotion } from '#lib/motion.ts';
	import PulsePlot from '#lib/components/PulsePlot.svelte';
	import CodeStrip from '#lib/components/CodeStrip.svelte';
	import Dial from '#lib/components/Dial.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { getDesk, previewFrame } from '#lib/api.ts';
	import { app } from '#lib/state.svelte.ts';
	import type { Config, DeskDevice, Effect, EffectKind, Rgb } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	const kinds: { value: EffectKind; label: string; code: string }[] = [
		{ value: 'wave', label: 'Wave', code: '101' },
		{ value: 'spectrum', label: 'Spectrum', code: '102' },
		{ value: 'static', label: 'Static', code: '103' },
		{ value: 'off', label: 'Off', code: '104' }
	];
	const current = $derived(kinds.find((k) => k.value === config.effect.kind)!);

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

	// One effect clock for the plot and the strips. Reduced motion starts paused.
	const reduce = reducedMotion();
	let paused = $state(reduce);
	let clock = 0;
	let lastNow = performance.now();
	const time = () => {
		const now = performance.now();
		if (!paused) clock += (now - lastNow) / 1000;
		lastNow = now;
		return clock;
	};

	// Desk geometry once; live LED colours from the real engine a few times a second for the strips.
	let desk = $state<DeskDevice[]>([]);
	let colors = $state<string[][]>([]);
	onMount(() => {
		let alive = true;
		getDesk($state.snapshot(config) as Config).then((d) => alive && (desk = d));
		const id = setInterval(async () => {
			if (!desk.length) return;
			const f = await previewFrame($state.snapshot(config) as Config, time());
			if (alive) colors = f;
		}, 120);
		return () => {
			alive = false;
			clearInterval(id);
		};
	});

	const ledCount = $derived(desk.filter((d) => d.kind !== 'mousemat').reduce((n, d) => n + d.shapes.length, 0) + desk.filter((d) => d.kind === 'mousemat').length);
	const liveIds = $derived(new Set(app.status?.devices.map((d) => d.id) ?? []));
	const engineUp = $derived(!!app.status);

	/** Up to 12 evenly spaced live colours per device, for its code strip. */
	function stripFor(i: number): string[] {
		const c = colors[i];
		if (!c?.length) return [];
		const n = Math.min(12, c.length);
		return Array.from({ length: n }, (_, k) => c[Math.floor(((k + 0.5) / n) * c.length)]);
	}

	const data = $derived.by(() => {
		const e = config.effect;
		const rows: [string, string][] = [['Effect', current.label]];
		if (e.kind === 'wave') rows.push(['Angle', `${e.angle_deg}°`], ['Cycle', fmtPeriod(e.period_s)], ['Band', `${e.wavelength} keys`], ['Travel', e.reverse ? 'Reversed' : 'Forward']);
		if (e.kind === 'spectrum') rows.push(['Cycle', fmtPeriod(e.period_s)]);
		if (e.kind === 'static') rows.push(['Colour', toHex(e.color).toUpperCase()]);
		rows.push(['Brightness', pct(config.brightness * 100)], ['Saturation', pct(config.saturation * 100)], ['Frame rate', `${config.fps} fps`], ['LEDs', String(ledCount || '—')]);
		return rows;
	});
</script>

<section class="lighting" aria-labelledby="lighting-title">
	<div class="stage">
		<header class="stage-head">
			<h1 id="lighting-title" class="title">
				<span class="display fac">FAC {current.code}</span>
				<span class="caps name">{current.label}</span>
			</h1>
			<button class="ghost" type="button" aria-pressed={paused} onclick={() => (paused = !paused)}>
				<span class="caps-sm">{paused ? 'Play preview' : 'Pause preview'}</span>
				<span class="glyph" aria-hidden="true">
					{#if paused}<svg viewBox="0 0 10 10"><path d="M2 1.5v7l6-3.5z" /></svg>{:else}<svg viewBox="0 0 10 10"><path d="M2.5 1.5h1.6v7H2.5zM5.9 1.5h1.6v7H5.9z" /></svg>{/if}
				</span>
			</button>
		</header>
		<div class="plot-area">
			{#if desk.length}
				<PulsePlot {desk} {config} {paused} {time} />
			{/if}
		</div>
		<footer class="stage-foot caps-sm">
			<span>Each line is a slice of your desk · pointer decodes a slice in live colour</span>
			<span class="num">{desk.length} devices on the desk</span>
		</footer>
	</div>

	<aside class="data" aria-label="Current effect">
		<h2 class="caps head">Transmission data</h2>
		<dl>
			{#each data as [k, v] (k)}
				<div class="row">
					<dt class="caps-sm">{k}</dt>
					<dd class="num">{v}</dd>
				</div>
			{/each}
		</dl>
	</aside>

	<div class="controls">
		<Segmented label="Effect" options={kinds} value={config.effect.kind} onchange={setKind} />
		<div class="params">
			{#key config.effect.kind}
				<div class="params-inner" in:fly={{ x: 14, duration: ms(380), easing: expoOut }} out:fade={{ duration: ms(120) }}>
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
							<Slider label="Speed" min={0} max={100} step={0.5} bind:value={() => toPos(spectrum.period_s), (v) => (spectrum.period_s = toPeriod(v))} format={(v) => `${fmtPeriod(toPeriod(v))} per cycle`} ends={['Slower', 'Faster']} hint="Every LED on the desk shows the same colour and moves through the rainbow together." />
						</div>
					{:else if config.effect.kind === 'static'}
						{@const stat = config.effect}
						<div class="col wide">
							<div class="color">
								<label class="caps-sm" for={colorId}>Colour</label>
								<div class="swatch-row">
									<input id={colorId} type="color" value={toHex(stat.color)} oninput={(e) => (stat.color = fromHex(e.currentTarget.value))} />
									<output class="num" for={colorId}>{toHex(stat.color).toUpperCase()}</output>
								</div>
							</div>
						</div>
					{:else}
						<p class="off-note">Lighting is off. Your devices stay dark until you choose another effect.</p>
					{/if}
				</div>
			{/key}
			<div class="col output">
				<Slider label="Brightness" min={0} max={100} bind:value={() => Math.round(config.brightness * 100), (v) => (config.brightness = v / 100)} format={pct} disabled={config.effect.kind === 'off'} />
				<Slider label="Saturation" min={0} max={100} bind:value={() => Math.round(config.saturation * 100), (v) => (config.saturation = v / 100)} format={pct} ends={['White', 'Vivid']} disabled={config.effect.kind === 'off' || config.effect.kind === 'static'} />
			</div>
		</div>
	</div>

	<div class="devices" aria-label="Devices on the desk">
		<h2 class="caps head">Devices</h2>
		<ul>
			{#each desk as d, i (d.id)}
				{@const live = liveIds.has(d.id)}
				<li class:away={engineUp && !live}>
					<CodeStrip colors={live || !engineUp ? stripFor(i) : []} label="{d.name} live colours" />
					<span class="dname">{d.name.replace(/^Razer /, '')}</span>
					<span class="dstate caps-sm">{!engineUp ? 'Preview' : live ? 'Live' : 'Away'}</span>
				</li>
			{/each}
		</ul>
	</div>
</section>

<style>
	.lighting {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 236px;
		grid-template-rows: minmax(300px, 1fr) auto;
		grid-template-areas:
			'stage data'
			'controls devices';
		gap: 0;
		height: 100%;
		min-height: 0;
		border: var(--hair);
	}
	.stage {
		grid-area: stage;
		display: grid;
		grid-template-rows: auto 1fr auto;
		min-height: 0;
		border-right: var(--hair);
		border-bottom: var(--hair);
	}
	.stage-head {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		padding: 22px 24px 0;
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
		white-space: nowrap;
	}
	.name {
		color: var(--color-ink-2);
		letter-spacing: 0.32em;
	}
	.ghost {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 8px 10px 8px 14px;
		border: var(--hair-strong);
		background: none;
		color: var(--color-ink-2);
		transition:
			color var(--t-mid) var(--ease),
			border-color var(--t-mid) var(--ease);
	}
	.ghost:hover {
		color: var(--color-ink);
		border-color: var(--color-ink-3);
	}
	.glyph svg {
		width: 10px;
		height: 10px;
		fill: currentColor;
		display: block;
	}
	.plot-area {
		min-height: 0;
		position: relative;
	}
	.stage-foot {
		display: flex;
		justify-content: space-between;
		gap: 16px;
		padding: 0 24px 14px;
		color: var(--color-ink-4);
	}

	.data {
		grid-area: data;
		padding: 22px 22px 18px;
		border-bottom: var(--hair);
		min-height: 0;
		overflow: auto;
	}
	.head {
		color: var(--color-ink-2);
		margin: 0 0 14px;
		padding-bottom: 10px;
		border-bottom: var(--hair);
		font-weight: 500;
	}
	dl {
		margin: 0;
		display: grid;
		gap: 12px;
	}
	.row {
		display: grid;
		gap: 3px;
	}
	dt {
		color: var(--color-ink-3);
	}
	dd {
		margin: 0;
		font-size: 15px;
		font-stretch: 112%;
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.controls {
		grid-area: controls;
		display: grid;
		align-content: start;
		gap: 22px;
		padding: 20px 24px 22px;
		border-right: var(--hair);
		min-height: 268px;
	}
	.params {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 0.62fr);
		gap: 32px;
		align-items: start;
	}
	.params-inner {
		grid-area: 1 / 1;
		display: grid;
		grid-template-columns: 128px minmax(0, 1fr);
		gap: 28px;
		align-items: start;
	}
	.col {
		display: grid;
		gap: 16px;
		min-width: 0;
	}
	.col.wide {
		grid-column: 1 / -1;
	}
	.output {
		grid-column: 2;
		grid-row: 1;
	}
	.off-note {
		grid-column: 1 / -1;
		color: var(--color-ink-3);
		font-size: 13px;
		max-width: 44ch;
		margin: 6px 0 0;
	}
	.color {
		display: grid;
		gap: 10px;
	}
	.color label {
		color: var(--color-ink-2);
	}
	.swatch-row {
		display: flex;
		align-items: center;
		gap: 14px;
	}
	.swatch-row output {
		font-size: 14px;
		letter-spacing: 0.06em;
	}
	input[type='color'] {
		appearance: none;
		width: 64px;
		height: 30px;
		padding: 0;
		border: var(--hair-strong);
		background: none;
	}
	input[type='color']::-webkit-color-swatch-wrapper {
		padding: 3px;
	}
	input[type='color']::-webkit-color-swatch {
		border: 0;
		border-radius: 0;
	}

	.devices {
		grid-area: devices;
		padding: 20px 22px;
		min-height: 0;
		overflow: auto;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 14px;
	}
	li {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 6px 10px;
		transition: opacity var(--t-slow) var(--ease);
	}
	li :global(.strip) {
		grid-column: 1 / -1;
	}
	.dname {
		font-size: 13px;
		color: var(--color-ink);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.dstate {
		color: var(--color-fac-red);
	}
	li.away .dname {
		color: var(--color-ink-3);
	}
	li.away .dstate {
		color: var(--color-ink-3);
	}

	/* Narrow windows (down to the 900px minimum): one column that scrolls. The transmission data repeats
	   what the controls already show, so it steps aside rather than squeezing them. */
	@container view (max-width: 820px) {
		.lighting {
			grid-template-columns: minmax(0, 1fr);
			grid-template-rows: 300px auto auto;
			grid-template-areas: 'stage' 'controls' 'devices';
			height: auto;
			min-height: 100%;
		}
		.data {
			display: none;
		}
		.stage,
		.controls {
			border-right: 0;
		}
		.controls {
			border-bottom: var(--hair);
		}
	}
</style>
