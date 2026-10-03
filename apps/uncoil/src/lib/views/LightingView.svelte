<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { Pause, Play } from '@lucide/svelte';
	import { ms, reducedMotion } from '#lib/motion.ts';
	import Workspace from '#lib/components/Workspace.svelte';
	import DeskPreview from '#lib/components/DeskPreview.svelte';
	import Dial from '#lib/components/Dial.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { getDesk, previewFrame } from '#lib/api.ts';
	import { app } from '#lib/state.svelte.ts';
	import type { Config, DeskDevice, Effect, EffectKind, Rgb } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	const toHex = ([r, g, b]: Rgb) => '#' + [r, g, b].map((c) => c.toString(16).padStart(2, '0')).join('');
	const fromHex = (h: string): Rgb => {
		const n = parseInt(h.slice(1), 16);
		return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
	};

	// The effects as swatch cards. Each card's swatch shows what the effect does to the desk.
	const kinds: { value: EffectKind; label: string; note: string }[] = [
		{ value: 'wave', label: 'Wave', note: 'A rainbow that moves across the whole desk' },
		{ value: 'spectrum', label: 'Spectrum', note: 'Every light fades through the rainbow together' },
		{ value: 'static', label: 'Static', note: 'One colour everywhere' },
		{ value: 'off', label: 'Off', note: 'Lights stay dark' }
	];
	const RAINBOW = '#ff0000, #ff8a00, #ffe600, #2bd94a, #00b3ff, #3a3aff, #b040ff, #ff0060';
	const swatchFor = (k: EffectKind) =>
		k === 'wave'
			? `linear-gradient(${config.effect.kind === 'wave' ? 90 + config.effect.angle_deg : 125}deg, ${RAINBOW})`
			: k === 'spectrum'
				? `linear-gradient(180deg, #ff0060 0 16%, #ff8a00 16% 33%, #ffe600 33% 50%, #2bd94a 50% 66%, #00b3ff 66% 83%, #b040ff 83%)`
				: k === 'static'
					? config.effect.kind === 'static'
						? toHex(config.effect.color)
						: toHex(remembered.static.kind === 'static' ? remembered.static.color : [226, 160, 62])
					: 'var(--case)';

	// Remember each effect's settings while you flip between them; seeded with the engine's defaults.
	const remembered: Record<EffectKind, Effect> = $state({
		wave: { kind: 'wave', angle_deg: 35, period_s: 14, wavelength: 26, reverse: false },
		spectrum: { kind: 'spectrum', period_s: 14 },
		static: { kind: 'static', color: [224, 163, 62] },
		off: { kind: 'off' }
	});
	function setKind(kind: EffectKind) {
		if (kind === config.effect.kind) return;
		remembered[config.effect.kind] = $state.snapshot(config.effect) as Effect;
		const next = structuredClone($state.snapshot(remembered[kind]) as Effect);
		if ('period_s' in next && 'period_s' in config.effect) next.period_s = config.effect.period_s;
		config.effect = next;
	}

	// Named colours for Static, like a gel book's swatches.
	const GELS: { name: string; hex: string }[] = [
		{ name: 'Warm white', hex: '#ffd9a8' },
		{ name: 'Cool white', hex: '#e6efff' },
		{ name: 'Straw', hex: '#f3d36b' },
		{ name: 'Amber', hex: '#e0a33e' },
		{ name: 'Primary red', hex: '#d7262e' },
		{ name: 'Rose pink', hex: '#e0559a' },
		{ name: 'Lavender', hex: '#9a7ce0' },
		{ name: 'Congo blue', hex: '#3a2fa0' },
		{ name: 'Steel blue', hex: '#4d7fb8' },
		{ name: 'Cyan', hex: '#26c6da' },
		{ name: 'Teal', hex: '#2a9d8f' },
		{ name: 'Moss green', hex: '#6aa84f' }
	];

	// Speed on a log scale: reads evenly from a slow drift (60 s per cycle) to a quick cycle (2 s).
	const SLOW = 60;
	const FAST = 2;
	const toPos = (period: number) => (100 * Math.log(period / SLOW)) / Math.log(FAST / SLOW);
	const toPeriod = (pos: number) => Math.round(SLOW * Math.pow(FAST / SLOW, pos / 100) * 10) / 10;
	const fmtPeriod = (p: number) => `${p < 10 ? p.toFixed(1) : Math.round(p)} s`;
	const pct = (v: number) => `${Math.round(v)}%`;
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
	const away = $derived(new Set(app.status ? desk.filter((d) => !liveIds.has(d.id)).map((d) => d.id) : []));
	const current = $derived(kinds.find((k) => k.value === config.effect.kind)!);
</script>

<Workspace title="Lighting" subtitle="One effect across your whole desk. Changes apply as you make them." panelLabel="Effect settings">
	{#snippet tools()}
		<button class="btn-quiet" type="button" aria-pressed={paused} onclick={() => (paused = !paused)}>
			{#if paused}<Play size={14} />Play preview{:else}<Pause size={14} />Pause preview{/if}
		</button>
	{/snippet}

	<div class="stage-desk">
		<DeskPreview {desk} {colors} {away} />
	</div>

	<section aria-labelledby="effect-title">
		<h2 id="effect-title" class="section-title">Effect</h2>
		<div class="cards" role="radiogroup" aria-labelledby="effect-title">
			{#each kinds as k (k.value)}
				<button type="button" class="card" role="radio" aria-checked={config.effect.kind === k.value} onclick={() => setKind(k.value)}>
					<span class="swatch" style:background={swatchFor(k.value)}></span>
					<span class="name">{k.label}</span>
					<span class="note">{k.note}</span>
				</button>
			{/each}
		</div>
	</section>

	{#snippet panel()}
		{#key config.effect.kind}
			<div class="settings" in:fade={{ duration: ms(160) }}>
				<h2 class="section-title">{current.label}</h2>
				{#if config.effect.kind === 'wave'}
					{@const wave = config.effect}
					<Dial label="Direction" bind:value={wave.angle_deg} />
					<Slider label="Speed" min={0} max={100} step={0.5} bind:value={() => toPos(wave.period_s), (v) => (wave.period_s = toPeriod(v))} format={(v) => `${fmtPeriod(toPeriod(v))} per cycle`} ends={['Slower', 'Faster']} />
					<Slider label="Band width" min={6} max={60} bind:value={wave.wavelength} format={(v) => `${v} keys`} ends={['Tight', 'Broad']} />
					<Toggle label="Reverse direction" bind:checked={wave.reverse} />
				{:else if config.effect.kind === 'spectrum'}
					{@const spectrum = config.effect}
					<Slider label="Speed" min={0} max={100} step={0.5} bind:value={() => toPos(spectrum.period_s), (v) => (spectrum.period_s = toPeriod(v))} format={(v) => `${fmtPeriod(toPeriod(v))} per cycle`} ends={['Slower', 'Faster']} />
				{:else if config.effect.kind === 'static'}
					{@const stat = config.effect}
					<div class="gels" role="radiogroup" aria-label="Colour">
						{#each GELS as g (g.hex)}
							<button type="button" class="gelchip" role="radio" aria-checked={toHex(stat.color) === g.hex} onclick={() => (stat.color = fromHex(g.hex))}>
								<span class="chip" style:background={g.hex}></span>{g.name}
							</button>
						{/each}
					</div>
					<div class="custom">
						<label class="label" for={colorId}>Or any colour</label>
						<input id={colorId} type="color" value={toHex(stat.color)} oninput={(e) => (stat.color = fromHex(e.currentTarget.value))} />
						<output class="num" for={colorId}>{toHex(stat.color).toUpperCase()}</output>
					</div>
				{:else}
					<p class="off-note">Your devices stay dark until you pick another effect.</p>
				{/if}

				{#if config.effect.kind !== 'off'}
					<div class="divider"></div>
					<Slider label="Brightness" min={0} max={100} bind:value={() => Math.round(config.brightness * 100), (v) => (config.brightness = v / 100)} format={pct} />
					{#if config.effect.kind !== 'static'}
						<Slider label="Saturation" min={0} max={100} bind:value={() => Math.round(config.saturation * 100), (v) => (config.saturation = v / 100)} format={pct} ends={['White', 'Vivid']} />
					{/if}
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
		max-height: 44vh;
	}
	section {
		display: grid;
		gap: 10px;
	}
	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
		gap: 12px;
	}
	.card {
		display: grid;
		gap: 4px;
		padding: 8px 8px 12px;
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
		height: 64px;
		margin-bottom: 6px;
		border-radius: var(--radius-sm);
		box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.08);
	}
	.name {
		padding: 0 4px;
		font-weight: 600;
		font-size: 13px;
	}
	.note {
		padding: 0 4px;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.35;
	}
	.settings {
		display: grid;
		gap: 18px;
	}
	.divider {
		height: 1px;
		background: var(--color-seam);
	}
	.gels {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 4px;
	}
	.gelchip {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 32px;
		padding: 0 8px 0 4px;
		border: 1px solid transparent;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-2);
		font-size: 12px;
		text-align: left;
	}
	.gelchip:hover {
		background: var(--color-surface-2);
	}
	.gelchip[aria-checked='true'] {
		border-color: var(--color-select);
		color: var(--color-ink);
		font-weight: 600;
	}
	.chip {
		width: 16px;
		height: 22px;
		border-radius: 3px;
		box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
		flex: none;
	}
	.custom {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.custom label {
		margin-right: auto;
	}
	.custom output {
		color: var(--color-ink-3);
		font-size: 12px;
	}
	input[type='color'] {
		appearance: none;
		width: 44px;
		height: 28px;
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
	.off-note {
		margin: 0;
		color: var(--color-ink-3);
	}
	@container view (max-width: 640px) {
		.cards {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
</style>
