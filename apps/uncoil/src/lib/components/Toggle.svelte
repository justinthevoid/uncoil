<script lang="ts">
	// On/off switch: label on the left, a pill switch on the right that fills with ink when on.
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
		onclick={(e) => {
			// A click on the inner spans still reaches Svelte's delegated handler when the button is disabled
			// (by a disabled fieldset, say), so check before flipping.
			if (!e.currentTarget.matches(':disabled')) checked = !checked;
		}}
	>
		<span class="text">{label}</span>
		<span class="track" class:on={checked} aria-hidden="true"><span class="knob"></span></span>
	</button>
	{#if hint}
		<p class="hint" id="{id}-hint">{hint}</p>
	{/if}
</div>

<style>
	.toggle {
		display: grid;
		gap: 6px;
	}
	button {
		display: grid;
		grid-template-columns: 1fr auto;
		align-items: center;
		gap: 16px;
		width: 100%;
		padding: 2px 0;
		background: none;
		border: 0;
		border-radius: var(--radius);
		text-align: left;
	}
	.text {
		font-size: 14px;
	}
	.track {
		position: relative;
		width: 38px;
		height: 22px;
		border-radius: 11px;
		background: var(--color-surface-3);
		transition: background-color var(--t-mid) var(--ease);
	}
	.knob {
		position: absolute;
		top: 3px;
		left: 3px;
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: var(--color-ink-2);
		box-shadow: 0 1px 2px rgb(0 0 0 / 0.5);
		transition:
			transform var(--t-slow) var(--ease),
			background-color var(--t-mid) var(--ease);
	}
	.on {
		background: var(--color-ink);
	}
	.on .knob {
		transform: translateX(16px);
		background: var(--color-ground);
	}
	button:hover .track:not(.on) {
		background: var(--color-seam-2);
	}
	.hint {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.45;
		max-width: 52ch;
	}
</style>
