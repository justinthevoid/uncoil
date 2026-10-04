<script lang="ts">
	// Shown where a feature needs uncoild's control pipe and it isn't answering (or the device is away).
	import { loadDevices } from '#lib/daemon.svelte.ts';
	import { pageSubject, type PageId } from '#lib/pages.ts';

	let { page, unreachable = true }: { page: PageId; unreachable?: boolean } = $props();
	const what = $derived(pageSubject(page));
</script>

<div class="unavail">
	{#if unreachable}
		<h1 class="page-title">The engine isn't answering</h1>
		<p>{what} talks to your devices through the uncoil engine, and it isn't responding. Install or update it with <code>scripts\install-task.ps1</code>, then try again.</p>
	{:else}
		<h1 class="page-title">This device isn't connected</h1>
		<p>{what} needs the device plugged in (or its receiver connected). It's picked up again within a few seconds of coming back.</p>
	{/if}
	<button type="button" class="btn-quiet" onclick={loadDevices}>Try again</button>
</div>

<style>
	.unavail {
		display: grid;
		gap: 12px;
		justify-items: start;
		max-width: 560px;
		padding: 28px;
	}
	p {
		margin: 0;
		color: var(--color-ink-2);
		line-height: 1.55;
	}
	code {
		font-family: ui-monospace, 'Cascadia Mono', Consolas, monospace;
		font-size: 12px;
		color: var(--color-ink);
	}
</style>
