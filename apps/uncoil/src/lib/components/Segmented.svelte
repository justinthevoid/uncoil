<script lang="ts" generics="T extends string">
	// Mutually exclusive choices (radio group) set as catalog entries: equal cells, hairline seams, and
	// one outlined indicator carrying the red code block that slides to the chosen entry.
	interface Props {
		label: string;
		options: { value: T; label: string; code?: string }[];
		value: T;
		onchange: (v: T) => void;
	}
	let { label, options, value, onchange }: Props = $props();

	let buttons: HTMLButtonElement[] = $state([]);
	const index = $derived(Math.max(0, options.findIndex((o) => o.value === value)));

	function onkeydown(e: KeyboardEvent, i: number) {
		const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
		if (!d) return;
		e.preventDefault();
		const j = (i + d + options.length) % options.length;
		onchange(options[j].value);
		buttons[j]?.focus();
	}
</script>

<div class="seg" role="radiogroup" aria-label={label} style:--n={options.length} style:--i={index}>
	<span class="indicator" aria-hidden="true"><span class="block"></span></span>
	{#each options as o, i (o.value)}
		<button
			bind:this={buttons[i]}
			type="button"
			role="radio"
			aria-checked={o.value === value}
			tabindex={o.value === value ? 0 : -1}
			onclick={() => onchange(o.value)}
			onkeydown={(e) => onkeydown(e, i)}
		>
			{#if o.code}<span class="code num">{o.code}</span>{/if}
			<span class="caps">{o.label}</span>
		</button>
	{/each}
</div>

<style>
	.seg {
		position: relative;
		display: grid;
		grid-template-columns: repeat(var(--n), 1fr);
		border: var(--hair-strong);
	}
	button {
		position: relative;
		z-index: 1;
		display: flex;
		align-items: baseline;
		gap: 10px;
		padding: 11px 14px 10px;
		border: 0;
		border-left: var(--hair);
		background: none;
		color: var(--color-ink-3);
		text-align: left;
		transition: color var(--t-mid) var(--ease);
	}
	button:first-of-type {
		border-left: 0;
	}
	button:hover {
		color: var(--color-ink-2);
	}
	button[aria-checked='true'] {
		color: var(--color-ink);
	}
	.code {
		font-size: 11px;
		font-stretch: 112%;
		letter-spacing: 0.08em;
		color: inherit;
		opacity: 0.7;
	}
	button[aria-checked='true'] .code {
		color: var(--color-fac-red);
		opacity: 1;
	}
	.caps {
		letter-spacing: 0.2em;
	}
	button:focus-visible {
		outline-offset: -4px;
	}
	/* The sliding catalog marker: an inked outline with the red code block on its trailing edge. */
	.indicator {
		position: absolute;
		inset: -1px auto -1px 0;
		width: calc(100% / var(--n));
		transform: translateX(calc(100% * var(--i)));
		border: var(--hair-ink);
		transition: transform var(--t-slow) var(--ease);
		pointer-events: none;
	}
	.block {
		position: absolute;
		right: 0;
		top: 0;
		bottom: 0;
		width: 6px;
		background: var(--color-fac-red);
		animation: block-in var(--t-slow) var(--ease);
	}
	@keyframes block-in {
		from {
			transform: scaleY(0);
		}
	}
</style>
