<script lang="ts">
	interface Props {
		label: string;
		checked: boolean;
		hint?: string;
	}
	let { label, checked = $bindable(), hint }: Props = $props();
	const id = $props.id();
</script>

<div class="toggle">
	<button
		{id}
		type="button"
		role="switch"
		aria-checked={checked}
		aria-describedby={hint ? `${id}-hint` : undefined}
		onclick={() => (checked = !checked)}
	>
		<span class="label">{label}</span>
		<span class="track" class:on={checked} aria-hidden="true"><span class="knob"></span></span>
	</button>
	{#if hint}
		<p class="hint" id="{id}-hint">{hint}</p>
	{/if}
</div>

<style>
	.toggle {
		display: grid;
		gap: 4px;
	}
	button {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		width: 100%;
		padding: 2px 0;
		background: none;
		border: 0;
		color: var(--color-text);
		font: inherit;
		font-size: 13px;
		text-align: left;
	}
	.track {
		flex: none;
		position: relative;
		width: 32px;
		height: 18px;
		border-radius: 9px;
		background: var(--color-ink-3);
		transition: background 120ms ease;
	}
	.knob {
		position: absolute;
		top: 3px;
		left: 3px;
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: var(--color-dim);
		transition:
			transform 120ms ease,
			background 120ms ease;
	}
	.on {
		background: var(--color-brass-dim);
	}
	.on .knob {
		transform: translateX(14px);
		background: var(--color-text);
	}
	.hint {
		color: var(--color-faint);
		font-size: 12px;
		line-height: 1.4;
		max-width: 52ch;
	}
	@media (prefers-reduced-motion: reduce) {
		.track,
		.knob {
			transition: none;
		}
	}
</style>
