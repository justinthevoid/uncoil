<script lang="ts">
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { app } from '#lib/state.svelte.ts';
	import type { Config } from '#lib/types.ts';

	let { config }: { config: Config } = $props();
	const d = $derived(config.display);
	const now = $derived(app.status?.display ?? null);
</script>

<section class="settings" aria-labelledby="settings-title">
	<header class="head">
		<h1 id="settings-title" class="page-title">Settings</h1>
		<p class="lede">
			{#if now}Your display is <strong>{now}</strong> and lighting is at {Math.round((app.status?.level ?? 1) * 100)}%.{:else}How lighting follows your monitor, and other RGB on your PC.{/if}
		</p>
	</header>

	<div class="card">
		<h2 class="section-title">Display</h2>
		<div class="rows">
			<Toggle label="Turn lighting off when the display sleeps" bind:checked={d.off_when_display_off} hint="Fades the desk to black when Windows turns the display off, and back when it wakes, like Synapse does." />
			<Slider label="Brightness while the display is dimmed" min={0} max={100} bind:value={() => Math.round(d.dim_level * 100), (v) => (d.dim_level = v / 100)} format={(v) => `${Math.round(v)}%`} hint="Windows dims the screen shortly before it sleeps. Lighting follows it down to this level." />
			<Slider label="Fade time" min={0} max={5} step={0.1} bind:value={d.fade_s} format={(v) => `${v.toFixed(1)} s`} hint="How long lighting takes to fade out or come back." />
		</div>
	</div>

	<div class="card">
		<h2 class="section-title">Other RGB</h2>
		<div class="rows">
			<Toggle label="Put motherboard, GPU and RAM on their built-in rainbow" bind:checked={config.openrgb_hardware_rainbow} hint="When the engine starts it runs OpenRGB once to set non-Razer RGB to its own rainbow, then OpenRGB exits. Needs OpenRGB installed." />
		</div>
	</div>
</section>

<style>
	.settings {
		display: grid;
		gap: 16px;
		align-content: start;
		max-width: 760px;
	}
	.lede {
		margin: 6px 0 0;
		color: var(--color-ink-3);
		font-size: 13px;
	}
	.lede strong {
		color: var(--color-ink);
		font-weight: 500;
	}
	.card {
		display: grid;
		gap: 16px;
		padding: 18px 20px;
		border-radius: var(--radius-lg);
		background: var(--color-raised);
		border: 1px solid var(--color-seam);
	}
	.rows {
		display: grid;
		gap: 22px;
	}
</style>
