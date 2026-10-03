<script lang="ts">
	interface Props {
		label: string;
		value: number;
		min: number;
		max: number;
		step?: number;
		/** Readout shown to the right of the label. */
		format?: (v: number) => string;
		/** Optional words under each end of the track, e.g. ['Slower', 'Faster']. */
		ends?: [string, string];
		hint?: string;
		disabled?: boolean;
	}

	let {
		label,
		value = $bindable(),
		min,
		max,
		step = 1,
		format = (v) => String(v),
		ends,
		hint,
		disabled = false
	}: Props = $props();

	const id = $props.id();
	const pct = $derived(((value - min) / (max - min)) * 100);
</script>

<div class="slider" class:disabled>
	<div class="head">
		<label class="caps-sm" for={id}>{label}</label>
		<output class="num" for={id}>{format(value)}</output>
	</div>
	<input
		{id}
		type="range"
		{min}
		{max}
		{step}
		{disabled}
		bind:value
		aria-valuetext={format(value)}
		style:--p="{pct}%"
	/>
	{#if ends}
		<div class="ends caps-sm" aria-hidden="true"><span>{ends[0]}</span><span>{ends[1]}</span></div>
	{/if}
	{#if hint}
		<p class="hint">{hint}</p>
	{/if}
</div>

<style>
	.slider {
		display: grid;
		gap: 4px;
	}
	.disabled {
		opacity: 0.35;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 12px;
	}
	label {
		color: var(--color-ink-2);
	}
	output {
		color: var(--color-ink);
		font-size: 13px;
		font-stretch: 105%;
	}
	.ends {
		display: flex;
		justify-content: space-between;
		color: var(--color-ink-4);
		font-size: 9px;
	}
	.hint {
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.45;
		max-width: 46ch;
	}

	/* Hairline track, white fill, a thin vertical tick for a thumb. */
	input[type='range'] {
		appearance: none;
		width: 100%;
		height: 22px;
		background: transparent;
		margin: 0;
	}
	input[type='range']::-webkit-slider-runnable-track {
		height: 1px;
		background: linear-gradient(to right, var(--color-ink) 0 var(--p), var(--color-seam-2) var(--p) 100%);
	}
	input[type='range']::-webkit-slider-thumb {
		appearance: none;
		width: 3px;
		height: 16px;
		margin-top: -7.5px;
		background: var(--color-ink);
		box-shadow: 0 0 0 4px var(--color-ground);
		transition:
			width var(--t-fast) var(--ease),
			background-color var(--t-fast) var(--ease);
	}
	input[type='range']:hover::-webkit-slider-thumb,
	input[type='range']:active::-webkit-slider-thumb {
		width: 5px;
	}
	input[type='range']:active::-webkit-slider-thumb {
		background: var(--color-fac-red);
	}
	input[type='range']:focus-visible {
		outline: none;
	}
	input[type='range']:focus-visible::-webkit-slider-thumb {
		box-shadow:
			0 0 0 3px var(--color-ground),
			0 0 0 4px var(--color-ink);
	}
</style>
