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
		<label for={id}>{label}</label>
		<output for={id}>{format(value)}</output>
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
		opacity: 0.4;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 12px;
	}
	label {
		color: var(--color-text);
		font-size: 13px;
	}
	output {
		color: var(--color-dim);
		font-size: 13px;
	}
	.ends {
		display: flex;
		justify-content: space-between;
		color: var(--color-faint);
		font-size: var(--text-2xs);
		margin-top: -2px;
	}
	.hint {
		color: var(--color-faint);
		font-size: 12px;
		line-height: 1.4;
	}

	input[type='range'] {
		appearance: none;
		width: 100%;
		height: 18px;
		background: transparent;
		margin: 0;
	}
	input[type='range']::-webkit-slider-runnable-track {
		height: 4px;
		border-radius: 2px;
		background: linear-gradient(
			to right,
			var(--color-brass-dim) 0 var(--p),
			var(--color-ink-3) var(--p) 100%
		);
	}
	input[type='range']::-webkit-slider-thumb {
		appearance: none;
		width: 14px;
		height: 14px;
		margin-top: -5px;
		border-radius: 50%;
		background: var(--color-text);
		box-shadow:
			0 0 0 3px var(--color-ink-1),
			0 1px 3px rgb(0 0 0 / 0.5);
	}
	input[type='range']:hover::-webkit-slider-thumb {
		background: #fff;
	}
	input[type='range']:focus-visible {
		outline: none;
	}
	input[type='range']:focus-visible::-webkit-slider-thumb {
		box-shadow:
			0 0 0 3px var(--color-ink-1),
			0 0 0 5px var(--color-brass);
	}
</style>
