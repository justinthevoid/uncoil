<script lang="ts">
	// On/off switch in the catalog world: a hairline rectangle with a square block that slides across
	// and turns into the red code block when on.
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
		<span class="state caps-sm" aria-hidden="true">{checked ? 'On' : 'Off'}</span>
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
		grid-template-columns: 1fr auto auto;
		align-items: center;
		gap: 14px;
		width: 100%;
		padding: 4px 0;
		background: none;
		border: 0;
		text-align: left;
	}
	.label {
		font-size: 14px;
	}
	.state {
		color: var(--color-ink-3);
		width: 3ch;
		text-align: right;
		transition: color var(--t-mid) var(--ease);
	}
	button[aria-checked='true'] .state {
		color: var(--color-ink);
	}
	.track {
		position: relative;
		width: 38px;
		height: 18px;
		border: var(--hair-strong);
		transition: border-color var(--t-mid) var(--ease);
	}
	.knob {
		position: absolute;
		top: 3px;
		left: 3px;
		width: 10px;
		height: 10px;
		background: var(--color-ink-4);
		transition:
			transform var(--t-slow) var(--ease),
			background-color var(--t-mid) var(--ease);
	}
	.on {
		border-color: var(--color-ink);
	}
	.on .knob {
		transform: translateX(20px);
		background: var(--color-fac-red);
	}
	button:hover .track:not(.on) {
		border-color: var(--color-ink-3);
	}
	.hint {
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.45;
		max-width: 52ch;
	}
</style>
