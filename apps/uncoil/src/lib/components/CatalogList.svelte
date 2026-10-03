<script lang="ts" generics="T extends string">
	// A vertical catalog index used as a radio group: code, name, optional note. The chosen row is boxed in
	// white with its code in red; `current` marks what the device holds now when that differs from the choice.
	interface Props {
		label: string;
		options: { value: T; label: string; code: string; note?: string }[];
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
			<span class="code num">{o.code}</span>
			<span class="name">{o.label}</span>
			{#if o.value === current}
				<span class="now caps-sm">On device</span>
			{:else if o.note}
				<span class="note caps-sm">{o.note}</span>
			{/if}
		</button>
	{/each}
</div>

<style>
	.list {
		display: grid;
	}
	button {
		display: grid;
		grid-template-columns: 40px 1fr auto;
		align-items: center;
		gap: 10px;
		height: 36px;
		padding: 0 12px;
		border: 1px solid transparent;
		border-bottom-color: var(--color-seam);
		background: none;
		color: var(--color-ink-3);
		text-align: left;
		transition:
			color var(--t-mid) var(--ease),
			border-color var(--t-mid) var(--ease);
	}
	button:hover {
		color: var(--color-ink-2);
	}
	button[aria-checked='true'] {
		color: var(--color-ink);
		border-color: var(--color-ink);
	}
	.code {
		font-size: 12px;
		font-stretch: 112%;
		letter-spacing: 0.06em;
		color: var(--color-ink-4);
		transition: color var(--t-mid) var(--ease);
	}
	button[aria-checked='true'] .code {
		color: var(--color-fac-red);
	}
	.name {
		font-size: 13px;
	}
	.now {
		color: var(--color-fac-red);
	}
	.note {
		color: var(--color-ink-4);
	}
	button:focus-visible {
		outline-offset: -3px;
	}
</style>
