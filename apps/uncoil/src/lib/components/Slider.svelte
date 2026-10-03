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
		<label class="label" for={id}>{label}</label>
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
		<div class="ends" aria-hidden="true"><span>{ends[0]}</span><span>{ends[1]}</span></div>
	{/if}
	{#if hint}
		<p class="hint">{hint}</p>
	{/if}
</div>

<style>
	.slider {
		display: grid;
		gap: 6px;
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
	output {
		color: var(--color-ink);
		font-size: 13px;
		font-weight: 500;
	}
	.ends {
		display: flex;
		justify-content: space-between;
		color: var(--color-ink-4);
		font-size: 11px;
	}
	.hint {
		margin: 2px 0 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.45;
		max-width: 46ch;
	}

	/* Rounded track with an ink fill, round thumb. */
	input[type='range'] {
		appearance: none;
		width: 100%;
		height: 20px;
		background: transparent;
		margin: 0;
	}
	input[type='range']::-webkit-slider-runnable-track {
		height: 4px;
		border-radius: 2px;
		background: linear-gradient(to right, var(--color-ink) 0 var(--p), var(--color-surface-3) var(--p) 100%);
	}
	input[type='range']::-webkit-slider-thumb {
		appearance: none;
		width: 16px;
		height: 16px;
		margin-top: -6px;
		border-radius: 50%;
		background: var(--color-ink);
		box-shadow:
			0 0 0 3px var(--color-ground),
			0 1px 3px 3px rgb(0 0 0 / 0.4);
		transition: transform var(--t-fast) var(--ease);
	}
	input[type='range']:hover::-webkit-slider-thumb {
		transform: scale(1.1);
	}
	input[type='range']:active::-webkit-slider-thumb {
		transform: scale(1.2);
	}
	input[type='range']:focus-visible {
		outline: none;
	}
	input[type='range']:focus-visible::-webkit-slider-thumb {
		box-shadow:
			0 0 0 3px var(--color-ground),
			0 0 0 5px var(--color-ink);
	}
</style>
