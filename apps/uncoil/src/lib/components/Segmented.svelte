<script lang="ts" generics="T extends string">
	// Mutually exclusive choices (radio group): equal segments on a dark track, with a raised pill that
	// slides to the chosen one.
	interface Props {
		label: string;
		options: { value: T; label: string }[];
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
	<span class="pill" aria-hidden="true"></span>
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
			{o.label}
		</button>
	{/each}
</div>

<style>
	.seg {
		position: relative;
		display: grid;
		grid-template-columns: repeat(var(--n), minmax(0, 1fr));
		padding: 3px;
		border-radius: calc(var(--radius) + 3px);
		background: var(--color-surface-2);
		border: 1px solid var(--color-seam);
	}
	button {
		position: relative;
		z-index: 1;
		height: 28px;
		padding: 0 10px;
		border: 0;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-3);
		font-size: 13px;
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		transition: color var(--t-mid) var(--ease);
	}
	button:hover {
		color: var(--color-ink-2);
	}
	button[aria-checked='true'] {
		color: var(--color-ink);
	}
	button:focus-visible {
		outline-offset: -2px;
	}
	.pill {
		position: absolute;
		top: 3px;
		bottom: 3px;
		left: 3px;
		width: calc((100% - 6px) / var(--n));
		transform: translateX(calc(100% * var(--i)));
		border-radius: var(--radius);
		background: var(--color-surface);
		box-shadow: 0 1px 2px rgb(0 0 0 / 0.18), 0 0 0 1px var(--color-seam);
		transition: transform var(--t-slow) var(--ease);
		pointer-events: none;
	}
</style>
