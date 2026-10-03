<script lang="ts">
	// A device's live colours as a coded strip of blocks (colour as code). With no colours (device away or
	// lighting off) it shows the grey FAC strip, so the state is readable without colour too.
	interface Props {
		colors: string[];
		label: string;
		blocks?: number;
	}
	let { colors, label, blocks = 12 }: Props = $props();
	const cells = $derived(Array.from({ length: blocks }, (_, i) => colors.length ? colors[Math.floor((i / blocks) * colors.length)] : null));
</script>

<div class="strip" role="img" aria-label={colors.length ? label : `${label}: none`}>
	{#each cells as c, i (i)}
		<span class="cell" class:empty={!c} style:background={c ?? undefined} style:transition-delay="{i * 18}ms"></span>
	{/each}
</div>

<style>
	.strip {
		display: grid;
		grid-auto-flow: column;
		grid-auto-columns: 1fr;
		gap: 3px;
		height: 10px;
	}
	.cell {
		background: var(--color-fac-grey);
		transition: background-color var(--t-slow) var(--ease);
	}
	.cell.empty {
		background: var(--color-seam-2);
	}
</style>
