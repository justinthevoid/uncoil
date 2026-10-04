<script lang="ts">
	// Pick a colour from named gels, or any colour; optionally "Rainbow" (null). Used by every effect that
	// takes a colour.
	import { GELS, fromHex, toHex } from '#lib/effects.ts';
	import type { Rgb } from '#lib/types.ts';

	interface Props {
		label: string;
		value: Rgb | null;
		/** Offer a "Rainbow" choice that sets null. */
		rainbow?: boolean;
		onchange: (c: Rgb | null) => void;
	}
	let { label, value, rainbow = false, onchange }: Props = $props();
	const id = $props.id();
	const cur = $derived(value ? toHex(value) : null);
</script>

<div class="picker" role="radiogroup" aria-label={label}>
	<p class="label">{label}</p>
	<div class="gels">
		{#if rainbow}
			<button type="button" class="gel-chip" role="radio" aria-checked={value === null} onclick={() => onchange(null)}>
				<span class="chip rainbow"></span>Rainbow
			</button>
		{/if}
		{#each GELS as g (g.hex)}
			<button type="button" class="gel-chip" role="radio" aria-checked={cur === g.hex} onclick={() => onchange(fromHex(g.hex))}>
				<span class="chip" style:background={g.hex}></span>{g.name}
			</button>
		{/each}
	</div>
	<div class="custom">
		<label class="label" for={id}>Or any colour</label>
		<input {id} type="color" value={cur ?? '#e0a33e'} oninput={(e) => onchange(fromHex(e.currentTarget.value))} />
		<output class="num" for={id}>{cur ? cur.toUpperCase() : 'Rainbow'}</output>
	</div>
</div>

<style>
	.picker {
		display: grid;
		gap: 8px;
	}
	.picker > .label {
		margin: 0;
	}
	.gels {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 2px 4px;
	}
	.gel-chip {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 30px;
		padding: 0 8px 0 4px;
		border: 1px solid transparent;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-2);
		font-size: 12px;
		text-align: left;
	}
	.gel-chip:hover {
		background: var(--color-surface-2);
	}
	.gel-chip[aria-checked='true'] {
		border-color: var(--color-select);
		color: var(--color-ink);
		font-weight: 600;
	}
	.chip {
		width: 14px;
		height: 20px;
		border-radius: 3px;
		box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
		flex: none;
	}
	.chip.rainbow {
		background: linear-gradient(180deg, #ff0060, #ff8a00, #ffe600, #2bd94a, #00b3ff, #b040ff);
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
</style>
