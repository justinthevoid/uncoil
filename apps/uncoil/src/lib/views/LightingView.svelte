<script lang="ts">
	import DeskPreview from '#lib/components/DeskPreview.svelte';
	import Dial from '#lib/components/Dial.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import type { Config, Effect, EffectKind, Rgb } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	const kinds: { value: EffectKind; label: string }[] = [
		{ value: 'wave', label: 'Wave' },
		{ value: 'spectrum', label: 'Spectrum' },
		{ value: 'static', label: 'Static' },
		{ value: 'off', label: 'Off' }
	];

	// Remember each effect's settings while you flip between them, so Wave → Static → Wave
	// comes back the way you left it. Seeded with the engine's defaults.
	const remembered: Record<EffectKind, Effect> = {
		wave: { kind: 'wave', angle_deg: 35, period_s: 14, wavelength: 26, reverse: false },
		spectrum: { kind: 'spectrum', period_s: 14 },
		static: { kind: 'static', color: [212, 180, 108] },
		off: { kind: 'off' }
	};

	function setKind(kind: EffectKind) {
		if (kind === config.effect.kind) return;
		remembered[config.effect.kind] = $state.snapshot(config.effect) as Effect;
		const next = structuredClone(remembered[kind]);
		// Carry the speed across when both effects have one.
		if ('period_s' in next && 'period_s' in config.effect) next.period_s = config.effect.period_s;
		config.effect = next;
	}

	// Speed: a log scale reads evenly from a slow drift (60 s per cycle) to a quick cycle (2 s).
	const SLOW = 60;
	const FAST = 2;
	const toPos = (period: number) => (100 * Math.log(period / SLOW)) / Math.log(FAST / SLOW);
	const toPeriod = (pos: number) => Math.round(SLOW * Math.pow(FAST / SLOW, pos / 100) * 10) / 10;
	const fmtPeriod = (pos: number) => {
		const p = toPeriod(pos);
		return `${p < 10 ? p.toFixed(1) : Math.round(p)} s per cycle`;
	};

	const pct = (v: number) => `${Math.round(v)}%`;

	const toHex = ([r, g, b]: Rgb) => '#' + [r, g, b].map((c) => c.toString(16).padStart(2, '0')).join('');
	const fromHex = (h: string): Rgb => {
		const n = parseInt(h.slice(1), 16);
		return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
	};

	const colorId = $props.id();
</script>

<section class="lighting" aria-label="Lighting">
	<DeskPreview {config} />

	<div class="panel">
		<div class="effect-row">
			<span class="group-label">Effect</span>
			<div class="effect-picker">
				<Segmented label="Effect" options={kinds} value={config.effect.kind} onchange={setKind} />
			</div>
		</div>

		{#if config.effect.kind === 'off'}
			<p class="off-note">
				Lighting is off. Your devices stay dark until you choose another effect.
			</p>
		{:else}
			<div class="controls kind-{config.effect.kind}">
				{#if config.effect.kind === 'wave'}
					{@const wave = config.effect}
					<div class="col dial-col">
						<Dial label="Direction" bind:value={wave.angle_deg} />
					</div>
					<div class="col">
						<Slider
							label="Speed"
							min={0}
							max={100}
							step={0.5}
							bind:value={() => toPos(wave.period_s), (v) => (wave.period_s = toPeriod(v))}
							format={fmtPeriod}
							ends={['Slower', 'Faster']}
						/>
						<Slider
							label="Band width"
							min={6}
							max={60}
							bind:value={wave.wavelength}
							format={(v) => `${v} keys per rainbow`}
							ends={['Tight', 'Broad']}
						/>
						<Toggle label="Reverse direction" bind:checked={wave.reverse} />
					</div>
				{:else if config.effect.kind === 'spectrum'}
					{@const spectrum = config.effect}
					<div class="col">
						<Slider
							label="Speed"
							min={0}
							max={100}
							step={0.5}
							bind:value={() => toPos(spectrum.period_s), (v) => (spectrum.period_s = toPeriod(v))}
							format={fmtPeriod}
							ends={['Slower', 'Faster']}
							hint="Every LED on the desk shows the same colour and moves through the rainbow together."
						/>
					</div>
				{:else if config.effect.kind === 'static'}
					{@const stat = config.effect}
					<div class="col">
						<div class="color">
							<label for={colorId}>Colour</label>
							<div class="swatch-row">
								<input
									id={colorId}
									type="color"
									value={toHex(stat.color)}
									oninput={(e) => (stat.color = fromHex(e.currentTarget.value))}
								/>
								<output class="num" for={colorId}>{toHex(stat.color).toUpperCase()}</output>
							</div>
						</div>
					</div>
				{/if}

				<div class="col output-col">
					<Slider
						label="Brightness"
						min={0}
						max={100}
						bind:value={() => Math.round(config.brightness * 100), (v) => (config.brightness = v / 100)}
						format={pct}
					/>
					{#if config.effect.kind !== 'static'}
						<Slider
							label="Saturation"
							min={0}
							max={100}
							bind:value={() => Math.round(config.saturation * 100), (v) => (config.saturation = v / 100)}
							format={pct}
							ends={['White', 'Vivid']}
						/>
					{/if}
				</div>
			</div>
		{/if}
	</div>
</section>

<style>
	.lighting {
		display: grid;
		grid-template-rows: minmax(260px, 1fr) auto;
		gap: 20px;
		height: 100%;
		min-height: 0;
	}
	.panel {
		display: grid;
		align-content: start;
		gap: 18px;
		/* Room for the Wave controls whatever the effect, so the preview doesn't jump when you switch. */
		min-height: 290px;
		padding: 18px 20px 20px;
		border-radius: 14px;
		background: var(--color-ink-1);
		box-shadow: inset 0 0 0 1px var(--color-line);
	}
	.effect-row {
		display: flex;
		align-items: center;
		gap: 20px;
	}
	.group-label {
		width: 132px;
		flex: none;
		color: var(--color-dim);
		font-size: 13px;
	}
	.effect-picker {
		width: min(420px, 100%);
	}
	.controls {
		display: grid;
		grid-template-columns: 132px 1fr 1fr;
		gap: 20px 36px;
		align-items: start;
	}
	/* Without the dial, controls still line up under the effect picker. */
	.kind-spectrum > .col:first-child,
	.kind-static > .col:first-child {
		grid-column-start: 2;
	}
	.col {
		display: grid;
		gap: 18px;
		min-width: 0;
	}
	.dial-col {
		margin-top: -2px;
	}
	.off-note {
		color: var(--color-dim);
		font-size: 13px;
		padding: 6px 0 4px 152px;
	}
	.color {
		display: grid;
		gap: 8px;
	}
	.color label {
		font-size: 13px;
	}
	.swatch-row {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.swatch-row output {
		color: var(--color-dim);
		font-size: 13px;
	}
	input[type='color'] {
		appearance: none;
		width: 56px;
		height: 32px;
		padding: 0;
		border: 0;
		border-radius: 7px;
		background: none;
		box-shadow: inset 0 0 0 1px var(--color-line);
	}
	input[type='color']::-webkit-color-swatch-wrapper {
		padding: 3px;
	}
	input[type='color']::-webkit-color-swatch {
		border: 0;
		border-radius: 5px;
	}
</style>
