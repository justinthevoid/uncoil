<script lang="ts">
	// Writing a device's onboard memory takes two deliberate clicks: the action, then Confirm in a row that
	// says what will change and how to undo it. Escape or Cancel backs out.
	import { fade } from 'svelte/transition';
	import { ms } from '#lib/motion.ts';

	interface Props {
		label: string;
		/** What the confirm row says, e.g. "Saved in the keyboard. Restore original undoes it." */
		warning: string;
		disabled?: boolean;
		busy?: boolean;
		onconfirm: () => void;
	}
	let { label, warning, disabled = false, busy = false, onconfirm }: Props = $props();
	let asking = $state(false);
	let confirmBtn: HTMLButtonElement | undefined = $state();

	$effect(() => {
		if (asking) confirmBtn?.focus();
	});
	$effect(() => {
		if (disabled) asking = false;
	});
</script>

<div class="write">
	{#if !asking}
		<button type="button" class="btn" disabled={disabled || busy} onclick={() => (asking = true)}>
			<span class="caps">{busy ? 'Writing…' : label}</span>
			<span class="block" aria-hidden="true"></span>
		</button>
	{:else}
		<div class="confirm" role="group" aria-label="Confirm onboard write" in:fade={{ duration: ms(160) }}>
			<p>{warning}</p>
			<div class="row">
				<button
					bind:this={confirmBtn}
					type="button"
					class="btn"
					onclick={() => {
						asking = false;
						onconfirm();
					}}
					onkeydown={(e) => e.key === 'Escape' && (asking = false)}
				>
					<span class="caps">Confirm</span>
					<span class="block" aria-hidden="true"></span>
				</button>
				<button type="button" class="ghost caps" onclick={() => (asking = false)}>Cancel</button>
			</div>
		</div>
	{/if}
</div>

<style>
	.write {
		display: grid;
	}
	.btn {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-self: start;
		height: 38px;
		padding: 0 40px 0 16px;
		border: var(--hair-ink);
		background: none;
		color: var(--color-ink);
		overflow: hidden;
	}
	.btn:disabled {
		border-color: var(--color-seam-2);
		color: var(--color-ink-4);
	}
	.block {
		position: absolute;
		right: 0;
		top: 0;
		bottom: 0;
		width: 26px;
		background: var(--color-fac-red);
		transform: scaleX(0.6923);
		transform-origin: right;
		transition: transform var(--t-mid) var(--ease);
	}
	.btn:hover:not(:disabled) .block {
		transform: none;
	}
	.btn:disabled .block {
		background: var(--color-seam-2);
	}
	.confirm {
		display: grid;
		gap: 12px;
		padding: 14px;
		border: var(--hair-strong);
	}
	.confirm p {
		margin: 0;
		color: var(--color-ink-2);
		font-size: 13px;
		line-height: 1.5;
		max-width: 52ch;
	}
	.row {
		display: flex;
		gap: 10px;
		align-items: center;
	}
	.ghost {
		height: 38px;
		padding: 0 16px;
		border: var(--hair-strong);
		background: none;
		color: var(--color-ink-2);
		transition:
			color var(--t-mid) var(--ease),
			border-color var(--t-mid) var(--ease);
	}
	.ghost:hover {
		color: var(--color-ink);
		border-color: var(--color-ink-3);
	}
</style>
