<script lang="ts">
	import Workspace from '#lib/components/Workspace.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { app } from '#lib/state.svelte.ts';
	import type { Config } from '#lib/types.ts';

	let { config }: { config: Config } = $props();
	const d = $derived(config.display);
	const now = $derived(app.status?.display ?? null);
</script>

<Workspace title="Display & RGB" subtitle={now ? `Your display is ${now}; lighting is at ${Math.round((app.status?.level ?? 1) * 100)}%.` : 'How lighting follows your monitor, and other RGB on your PC.'}>
	<section class="card" aria-labelledby="display-title">
		<h2 id="display-title" class="section-title">When the display sleeps or dims</h2>
		<Toggle label="Turn lighting off when the display sleeps" bind:checked={d.off_when_display_off} hint="Fades the desk out when Windows turns the display off, and back when it wakes, like Synapse does." />
		<Slider label="Brightness while the display is dimmed" min={0} max={100} bind:value={() => Math.round(d.dim_level * 100), (v) => (d.dim_level = v / 100)} format={(v) => `${Math.round(v)}%`} hint="Windows dims the screen shortly before it sleeps. Lighting follows it down to this level." />
		<Slider label="Fade time" min={0} max={5} step={0.1} bind:value={d.fade_s} format={(v) => `${v.toFixed(1)} s`} hint="How long lighting takes to fade out or come back." />
	</section>
	<section class="card" aria-labelledby="rgb-title">
		<h2 id="rgb-title" class="section-title">Other RGB on this PC</h2>
		<Toggle label="Put motherboard, GPU and RAM on their built-in rainbow" bind:checked={config.openrgb_hardware_rainbow} hint="When the engine starts it runs OpenRGB once to set non-Razer RGB to its own rainbow, then OpenRGB exits. Needs OpenRGB installed." />
	</section>
</Workspace>

<style>
	.card {
		display: grid;
		gap: 20px;
		max-width: 720px;
		padding: 18px 20px;
		border: var(--hair);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
	}
</style>
