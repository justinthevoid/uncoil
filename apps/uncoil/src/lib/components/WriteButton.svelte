<script lang="ts">
	// Writing a device's onboard memory takes two deliberate clicks: the action, then Confirm in a panel
	// that says what will change and how to undo it. Escape or Cancel backs out.
	import { fade } from 'svelte/transition';
	import { ms } from '#lib/motion.ts';

	interface Props {
		label: string;
		/** What the confirm panel says, e.g. "Saved in the keyboard. Restore original undoes it." */
		warning: string;
		disabled?: boolean;
		busy?: boolean;
		/** Quiet (outline) style for secondary actions. */
		quiet?: boolean;
		onconfirm: () => void;
	}
	let { label, warning, disabled = false, busy = false, quiet = false, onconfirm }: Props = $props();
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
		<button type="button" class={quiet ? 'btn-quiet' : 'btn'} disabled={disabled || busy} onclick={() => (asking = true)}>
			{busy ? 'Saving…' : label}
		</button>
	{:else}
		<div class="confirm" role="group" aria-label="Confirm" in:fade={{ duration: ms(160) }}>
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
					Confirm
				</button>
				<button type="button" class="btn-quiet" onclick={() => (asking = false)}>Cancel</button>
			</div>
		</div>
	{/if}
</div>

<style>
	.write {
		display: grid;
		justify-items: start;
	}
	.confirm {
		display: grid;
		gap: 12px;
		width: 100%;
		padding: 14px;
		border-radius: var(--radius-lg);
		background: var(--color-surface-2);
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
		gap: 8px;
	}
</style>
