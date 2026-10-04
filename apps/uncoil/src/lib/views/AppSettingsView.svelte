<script lang="ts">
	// The app's own preferences: the tray icon, starting with Windows and battery notifications. They live in
	// the app's settings file (src-tauri `AppSettings`), not in the engine's config.json.
	import { onMount } from 'svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import { getAppSettings, saveAppSettings } from '#lib/api.ts';
	import type { AppSettings } from '#lib/types.ts';

	let settings = $state<AppSettings | null>(null);
	let error = $state<string | null>(null);
	let saved = '';

	onMount(async () => {
		try {
			const s = await getAppSettings();
			saved = JSON.stringify(s);
			settings = s;
		} catch (e) {
			error = String(e);
		}
	});

	// Save each change straight away (the tray and the battery check read the file).
	$effect(() => {
		if (!settings) return;
		const json = JSON.stringify(settings);
		if (json === saved) return;
		saved = json;
		saveAppSettings(JSON.parse(json))
			.then(() => (error = null))
			.catch((e) => (error = String(e)));
	});

	const pct = (v: number) => `${Math.round(v)}%`;
</script>

<Workspace title={pageTitle('app')} subtitle="How the uncoil window behaves. The engine keeps your lighting running either way.">
	{#if settings}
		<section class="card" aria-labelledby="tray-title">
			<h2 id="tray-title" class="section-title">Tray icon</h2>
			<Toggle
				label="Keep uncoil in the tray when the window closes"
				bind:checked={settings.close_to_tray}
				hint="Closing the window hides it instead of quitting. The tray icon opens it again and switches the desk effect; Quit is in its menu."
			/>
			<Toggle label="Start in the tray when Windows starts" bind:checked={settings.start_in_tray} hint="Opens hidden in the tray when you sign in, and stays there when you close the window." />
		</section>
		<section class="card" aria-labelledby="battery-title">
			<h2 id="battery-title" class="section-title">Battery notifications</h2>
			<Toggle
				label="Tell me when a wireless device needs charging"
				bind:checked={settings.battery_notifications}
				hint="Checks every 10 minutes while uncoil is open or in the tray. One notification at the level below, one more at 10%, and one when charging reaches 100%."
			/>
			<fieldset disabled={!settings.battery_notifications}>
				<Slider label="Warn at" min={15} max={50} step={5} bind:value={settings.battery_threshold} format={pct} ends={['15%', '50%']} disabled={!settings.battery_notifications} />
			</fieldset>
		</section>
	{:else if !error}
		<p class="hint">Loading…</p>
	{/if}
	{#if error}<p class="outcome bad" role="alert">Couldn't save these settings. {error}</p>{/if}
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
	fieldset {
		min-width: 0;
		margin: 0;
		padding: 0;
		border: 0;
		transition: opacity var(--t-mid) var(--ease);
	}
	fieldset:disabled {
		opacity: 0.45;
	}
	fieldset :global(.slider.disabled) {
		opacity: 1;
	}
	.hint {
		margin: 0;
		color: var(--color-ink-3);
	}
</style>
