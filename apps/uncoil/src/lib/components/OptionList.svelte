<script lang="ts" generics="T extends string">
	// A vertical list of choices used as a radio group, for lists too long for a segmented control.
	// `current` marks what the device holds now when that differs from the choice.
	interface Props {
		label: string;
		options: { value: T; label: string; note?: string }[];
		value: T | null;
		current?: T | null;
		onchange: (v: T) => void;
	}
	let { label, options, value, current = null, onchange }: Props = $props();
	let buttons: HTMLButtonElement[] = $state([]);

	function onkeydown(e: KeyboardEvent, i: number) {
		const d = e.key === 'ArrowDown' || e.key === 'ArrowRight' ? 1 : e.key === 'ArrowUp' || e.key === 'ArrowLeft' ? -1 : 0;
		if (!d) return;
		e.preventDefault();
		const j = (i + d + options.length) % options.length;
		onchange(options[j].value);
		buttons[j]?.focus();
	}
</script>

<div class="list" role="radiogroup" aria-label={label}>
	{#each options as o, i (o.value)}
		<button
			bind:this={buttons[i]}
			type="button"
			role="radio"
			aria-checked={o.value === value}
			tabindex={o.value === value || (value === null && i === 0) ? 0 : -1}
			onclick={() => onchange(o.value)}
			onkeydown={(e) => onkeydown(e, i)}
		>
			<span class="dot" aria-hidden="true"></span>
			<span class="name">{o.label}</span>
			{#if o.value === current}
				<span class="tag now">On device</span>
			{:else if o.note}
				<span class="tag">{o.note}</span>
			{/if}
		</button>
	{/each}
</div>

<style>
	.list {
		display: grid;
		gap: 2px;
	}
	button {
		display: grid;
		grid-template-columns: 16px 1fr auto;
		align-items: center;
		gap: 10px;
		height: 36px;
		padding: 0 12px;
		border: 0;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-2);
		text-align: left;
		font-size: 13px;
		transition:
			background-color var(--t-mid) var(--ease),
			color var(--t-mid) var(--ease);
	}
	button:hover {
		background: var(--color-surface);
		color: var(--color-ink);
	}
	button[aria-checked='true'] {
		background: var(--color-surface-2);
		color: var(--color-ink);
	}
	.dot {
		width: 14px;
		height: 14px;
		border-radius: 50%;
		border: 1.5px solid var(--color-ink-4);
		transition:
			border-color var(--t-mid) var(--ease),
			border-width var(--t-mid) var(--ease);
	}
	button[aria-checked='true'] .dot {
		border: 4.5px solid var(--color-select);
	}
	.tag {
		font-size: 11px;
		color: var(--color-ink-4);
	}
	.tag.now {
		color: var(--color-ink-2);
	}
	button:focus-visible {
		outline-offset: -2px;
	}
</style>
