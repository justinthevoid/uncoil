<script lang="ts">
	import { onMount } from 'svelte';
	import { app, loadConfig, pollStatus, scheduleSave } from '#lib/state.svelte.ts';
	import LightingView from '#lib/views/LightingView.svelte';
	import DevicesView from '#lib/views/DevicesView.svelte';
	import DisplayView from '#lib/views/DisplayView.svelte';
	import AboutView from '#lib/views/AboutView.svelte';

	type View = 'lighting' | 'devices' | 'display' | 'about';
	const nav: { id: View; label: string }[] = [
		{ id: 'lighting', label: 'Lighting' },
		{ id: 'devices', label: 'Devices' },
		{ id: 'display', label: 'Display' },
		{ id: 'about', label: 'About' }
	];
	let view = $state<View>('lighting');

	// The mark: the same unwinding spiral as the app icon.
	const spiral = Array.from({ length: 121 }, (_, i) => {
		const t = i / 120;
		const a = t * 2.25 * 2 * Math.PI;
		const r = 1.4 + t * 8.6;
		return `${i ? 'L' : 'M'}${(12 + r * Math.cos(a)).toFixed(2)} ${(12 + r * Math.sin(a)).toFixed(2)}`;
	}).join('');

	onMount(() => {
		loadConfig();
		pollStatus();
		const id = setInterval(pollStatus, 2000);
		return () => clearInterval(id);
	});

	// Persist every edit (debounced). The first snapshot after loading is the baseline, not an edit.
	let saved = '';
	$effect(() => {
		if (!app.config) return;
		const json = JSON.stringify(app.config);
		if (!saved) {
			saved = json;
		} else if (json !== saved) {
			saved = json;
			scheduleSave(JSON.parse(json));
		}
	});

	const engine = $derived.by(() => {
		if (!app.statusKnown) return { tone: 'unknown', title: 'Checking engine', detail: '' };
		const s = app.status;
		if (!s) return { tone: 'off', title: 'Engine not running', detail: 'Settings apply when it starts' };
		const n = s.devices.length;
		const fps = n ? Math.round(Math.max(...s.devices.map((d) => d.fps))) : 0;
		const devices = `${n} ${n === 1 ? 'device' : 'devices'}`;
		return {
			tone: 'on',
			title: 'Engine running',
			detail: s.display === 'off' ? `${devices}, display asleep` : `${devices} at ${fps} fps`
		};
	});
</script>

<div class="app">
	<nav aria-label="Sections">
		<div class="brand">
			<svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
				<path d={spiral} />
			</svg>
			<span>uncoil</span>
		</div>

		<ul>
			{#each nav as item (item.id)}
				<li>
					<button
						type="button"
						aria-current={view === item.id ? 'page' : undefined}
						onclick={() => (view = item.id)}>{item.label}</button
					>
				</li>
			{/each}
		</ul>

		<div class="engine" role="status" aria-live="polite">
			<span class="led {engine.tone}" aria-hidden="true"></span>
			<span class="engine-text">
				<span class="engine-title">{engine.title}</span>
				{#if engine.detail}<span class="engine-detail num">{engine.detail}</span>{/if}
			</span>
		</div>
		{#if app.saveError}
			<p class="save-error" role="alert">Couldn't save settings. {app.saveError}</p>
		{/if}
	</nav>

	<main>
		{#if app.loadError}
			<p class="load-error" role="alert">Couldn't read your settings: {app.loadError}</p>
		{:else if !app.config}
			<p class="loading">Loading settings…</p>
		{:else if view === 'lighting'}
			<LightingView config={app.config} />
		{:else if view === 'devices'}
			<DevicesView />
		{:else if view === 'display'}
			<DisplayView config={app.config} />
		{:else}
			<AboutView />
		{/if}
	</main>
</div>

<style>
	.app {
		display: grid;
		grid-template-columns: 188px 1fr;
		height: 100vh;
	}
	nav {
		display: flex;
		flex-direction: column;
		gap: 24px;
		padding: 22px 14px 18px;
		background: var(--color-ink-0);
		border-right: 1px solid var(--color-line);
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 0 10px;
		font-family: var(--font-display);
		font-size: 18px;
		font-weight: 600;
		letter-spacing: -0.01em;
	}
	.brand svg {
		fill: none;
		stroke: var(--color-brass);
		stroke-width: 2;
		stroke-linecap: round;
	}
	ul {
		display: grid;
		gap: 2px;
		list-style: none;
		margin: 0;
		padding: 0;
	}
	ul button {
		position: relative;
		width: 100%;
		padding: 8px 10px 8px 14px;
		border: 0;
		border-radius: 7px;
		background: transparent;
		color: var(--color-dim);
		font: inherit;
		font-size: 14px;
		text-align: left;
	}
	ul button:hover {
		color: var(--color-text);
		background: rgb(255 255 255 / 0.025);
	}
	ul button[aria-current='page'] {
		color: var(--color-text);
		background: var(--color-ink-1);
	}
	ul button[aria-current='page']::before {
		content: '';
		position: absolute;
		left: 4px;
		top: 9px;
		bottom: 9px;
		width: 2px;
		border-radius: 1px;
		background: var(--color-brass);
	}
	.engine {
		margin-top: auto;
		display: flex;
		align-items: flex-start;
		gap: 10px;
		padding: 10px;
		border-radius: 8px;
		background: var(--color-ink-1);
		box-shadow: inset 0 0 0 1px var(--color-line);
	}
	.led {
		flex: none;
		width: 8px;
		height: 8px;
		margin-top: 6px;
		border-radius: 50%;
		background: var(--color-faint);
	}
	.led.on {
		background: var(--color-ok);
		box-shadow: 0 0 6px rgb(143 185 154 / 0.6);
	}
	.led.off {
		background: transparent;
		box-shadow: inset 0 0 0 1.5px var(--color-faint);
	}
	.engine-text {
		display: grid;
		gap: 1px;
		min-width: 0;
	}
	.engine-title {
		font-size: 13px;
	}
	.engine-detail {
		color: var(--color-faint);
		font-size: 12px;
	}
	.save-error {
		color: var(--color-warn);
		font-size: 12px;
		padding: 0 4px;
	}
	main {
		min-width: 0;
		min-height: 0;
		padding: 22px 24px 22px;
		overflow: auto;
	}
	.loading,
	.load-error {
		color: var(--color-dim);
		font-size: 13px;
	}
	.load-error {
		color: var(--color-warn);
	}
</style>
