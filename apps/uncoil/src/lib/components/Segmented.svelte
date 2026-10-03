<script lang="ts" generics="T extends string">
	// A row of mutually exclusive choices (radio group), arrow-key navigable.
	interface Props {
		label: string;
		options: { value: T; label: string }[];
		value: T;
		onchange: (v: T) => void;
	}
	let { label, options, value, onchange }: Props = $props();

	let buttons: HTMLButtonElement[] = $state([]);

	function onkeydown(e: KeyboardEvent, i: number) {
		const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
		if (!d) return;
		e.preventDefault();
		const j = (i + d + options.length) % options.length;
		onchange(options[j].value);
		buttons[j]?.focus();
	}
</script>

<div class="seg" role="radiogroup" aria-label={label}>
	{#each options as o, i (o.value)}
		<button
			bind:this={buttons[i]}
			type="button"
			role="radio"
			aria-checked={o.value === value}
			tabindex={o.value === value ? 0 : -1}
			onclick={() => onchange(o.value)}
			onkeydown={(e) => onkeydown(e, i)}>{o.label}</button
		>
	{/each}
</div>

<style>
	.seg {
		display: grid;
		grid-auto-flow: column;
		grid-auto-columns: 1fr;
		gap: 2px;
		padding: 3px;
		border-radius: 9px;
		background: var(--color-well);
		box-shadow: inset 0 0 0 1px var(--color-line);
	}
	button {
		padding: 6px 12px;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: var(--color-dim);
		font: inherit;
		font-size: 13px;
	}
	button:hover {
		color: var(--color-text);
	}
	button[aria-checked='true'] {
		background: var(--color-ink-2);
		color: var(--color-text);
		box-shadow:
			inset 0 1px 0 rgb(255 255 255 / 0.04),
			0 1px 2px rgb(0 0 0 / 0.4);
	}
	button:focus-visible {
		outline-offset: -2px;
	}
</style>
