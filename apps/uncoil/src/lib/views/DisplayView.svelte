<script lang="ts">
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { app } from '#lib/state.svelte.ts';
	import type { Config } from '#lib/types.ts';

	let { config }: { config: Config } = $props();
	const d = $derived(config.display);
	const now = $derived(app.status?.display ?? null);
</script>

<section class="display-view" aria-labelledby="display-title">
	<header class="head">
		<h1 id="display-title" class="title"><span class="display fac">FAC 500</span><span class="caps name">Display</span></h1>
		<p class="lede">
			{#if now}Windows reports the display as <strong>{now}</strong>; lighting is at {Math.round((app.status?.level ?? 1) * 100)}%.{:else}How lighting follows your monitor's power state.{/if}
		</p>
	</header>

	<div class="grid">
		<section class="block" aria-labelledby="sleep-title">
			<h2 id="sleep-title" class="caps sub">When the display sleeps</h2>
			<Toggle label="Turn lighting off" bind:checked={d.off_when_display_off} hint="Fades the desk to black when Windows turns the display off, and back when it wakes. Synapse does the same." />
		</section>

		<section class="block" aria-labelledby="dim-title">
			<h2 id="dim-title" class="caps sub">When the display dims</h2>
			<Slider label="Brightness while dimmed" min={0} max={100} bind:value={() => Math.round(d.dim_level * 100), (v) => (d.dim_level = v / 100)} format={(v) => `${Math.round(v)}%`} hint="Windows dims the screen shortly before it sleeps. Lighting follows it down to this level." />
		</section>

		<section class="block" aria-labelledby="fade-title">
			<h2 id="fade-title" class="caps sub">Fade</h2>
			<Slider label="Fade time" min={0} max={5} step={0.1} bind:value={d.fade_s} format={(v) => `${v.toFixed(1)} s`} hint="How long lighting takes to fade out or come back." />
		</section>

		<section class="block" aria-labelledby="other-title">
			<h2 id="other-title" class="caps sub">Other RGB</h2>
			<Toggle label="Hand motherboard, GPU and RAM to their hardware rainbow" bind:checked={config.openrgb_hardware_rainbow} hint="At start, runs OpenRGB once to put non-Razer RGB on its own built-in rainbow, then OpenRGB exits. Needs OpenRGB installed." />
		</section>
	</div>
</section>

<style>
	.display-view {
		display: grid;
		grid-template-rows: auto 1fr;
		height: 100%;
		border: var(--hair);
	}
	.head {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: 24px;
		padding: 22px 24px 18px;
		border-bottom: var(--hair);
	}
	.title {
		display: flex;
		align-items: baseline;
		gap: 18px;
		margin: 0;
		font-weight: inherit;
	}
	.fac {
		font-size: 44px;
	}
	.name {
		color: var(--color-ink-2);
		letter-spacing: 0.32em;
	}
	.lede {
		margin: 0;
		color: var(--color-ink-2);
		font-size: 13px;
		max-width: 52ch;
		text-align: right;
	}
	.lede strong {
		color: var(--color-ink);
		font-weight: 500;
		text-transform: uppercase;
		letter-spacing: 0.12em;
		font-size: 12px;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		align-content: start;
	}
	.block {
		display: grid;
		gap: 16px;
		align-content: start;
		padding: 24px;
		border-bottom: var(--hair);
	}
	.block:nth-child(odd) {
		border-right: var(--hair);
	}
	.sub {
		margin: 0;
		color: var(--color-ink-2);
		font-weight: 500;
	}
</style>
