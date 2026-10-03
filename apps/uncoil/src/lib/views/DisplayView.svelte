<script lang="ts">
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { app } from '#lib/state.svelte.ts';
	import type { Config } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	const now: Record<string, string> = {
		on: 'Your display is on right now.',
		dimmed: 'Windows has dimmed your display right now.',
		off: 'Your display is asleep right now.'
	};
</script>

<section class="view" aria-labelledby="display-title">
	<header>
		<h1 id="display-title">Display</h1>
		<p class="sub">
			Lighting follows your screen, so the desk goes dark when you walk away.
			{#if app.status}{now[app.status.display] ?? ''}{/if}
		</p>
	</header>

	<div class="panel">
		<Toggle
			label="Turn lighting off when the display sleeps"
			bind:checked={config.display.off_when_display_off}
			hint="When Windows turns the screen off, lighting fades out. It comes back when the screen wakes."
		/>
		<hr />
		<Slider
			label="Brightness while the display is dimmed"
			min={0}
			max={100}
			bind:value={
				() => Math.round(config.display.dim_level * 100),
				(v) => (config.display.dim_level = v / 100)
			}
			format={(v) => `${v}% of normal`}
			hint="Windows dims the screen shortly before it turns it off. Lighting dims along with it."
		/>
		<hr />
		<Slider
			label="Fade time"
			min={0}
			max={5}
			step={0.1}
			bind:value={config.display.fade_s}
			format={(v) => (v === 0 ? 'Instant' : `${v.toFixed(1)} s`)}
			hint="How long lighting takes to fade between on, dimmed and off."
		/>
	</div>
</section>

<style>
	.view {
		display: grid;
		align-content: start;
		gap: 24px;
		max-width: 620px;
	}
	header {
		display: grid;
		gap: 6px;
	}
	h1 {
		font-family: var(--font-display);
		font-size: 24px;
		font-weight: 600;
		letter-spacing: -0.01em;
	}
	.sub {
		color: var(--color-dim);
		font-size: 13px;
		max-width: 62ch;
	}
	.panel {
		display: grid;
		gap: 18px;
		padding: 20px 22px;
		border-radius: 14px;
		background: var(--color-ink-1);
		box-shadow: inset 0 0 0 1px var(--color-line);
	}
	hr {
		border: 0;
		border-top: 1px solid var(--color-line);
		margin: 0;
	}
</style>
