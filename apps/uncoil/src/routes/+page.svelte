<script lang="ts">
	import { onMount, type Component } from 'svelte';
	import { fly } from 'svelte/transition';
	import { expoOut } from 'svelte/easing';
	import { Cpu, Info, Keyboard, Lightbulb, Settings, Usb } from '@lucide/svelte';
	import { app, loadConfig, pollStatus, scheduleSave } from '#lib/state.svelte.ts';
	import { ms } from '#lib/motion.ts';
	import LightingView from '#lib/views/LightingView.svelte';
	import KeysView from '#lib/views/KeysView.svelte';
	import HardwareView from '#lib/views/HardwareView.svelte';
	import DevicesView from '#lib/views/DevicesView.svelte';
	import SettingsView from '#lib/views/SettingsView.svelte';
	import AboutView from '#lib/views/AboutView.svelte';

	type View = 'lighting' | 'keys' | 'hardware' | 'devices' | 'settings' | 'about';
	const nav: { id: View; label: string; icon: Component<{ size?: number; strokeWidth?: number }> }[] = [
		{ id: 'lighting', label: 'Lighting', icon: Lightbulb },
		{ id: 'keys', label: 'Keys', icon: Keyboard },
		{ id: 'hardware', label: 'Dial & screen', icon: Cpu },
		{ id: 'devices', label: 'Devices', icon: Usb },
		{ id: 'settings', label: 'Settings', icon: Settings },
		{ id: 'about', label: 'About', icon: Info }
	];
	let view = $state<View>('lighting');
	const index = $derived(nav.findIndex((n) => n.id === view));

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
		if (!saved) saved = json;
		else if (json !== saved) {
			saved = json;
			scheduleSave(JSON.parse(json));
		}
	});

	const mb = (b: number) => `${(b / 1048576).toFixed(1)} MB`;
	const engine = $derived.by(() => {
		if (!app.statusKnown) return { state: 'checking', text: 'Checking…' };
		const s = app.status;
		if (!s) return { state: 'stopped', text: 'Engine not running' };
		const parts = [`${s.devices.length} device${s.devices.length === 1 ? '' : 's'}`];
		if (s.memory_bytes) parts.push(mb(s.memory_bytes), `${s.cpu_percent.toFixed(1)}% CPU`);
		return { state: 'running', text: parts.join(' · ') };
	});

	// One keyboard shortcut per section: Ctrl+1..6
	function onkeydown(e: KeyboardEvent) {
		if (!e.ctrlKey || e.altKey || e.metaKey) return;
		const n = Number(e.key);
		if (n >= 1 && n <= nav.length) {
			view = nav[n - 1].id;
			e.preventDefault();
		}
	}
</script>

<svelte:window {onkeydown} />

<div class="app">
	<nav aria-label="Sections">
		<div class="brand">
			<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M12 12a2 2 0 1 1 2-2 4 4 0 1 1-4-4 6 6 0 1 1-6 6 8 8 0 1 1 8 8" /></svg>
			<span class="wordmark">uncoil</span>
		</div>

		<ul style:--i={index}>
			<span class="marker" aria-hidden="true"></span>
			{#each nav as item, i (item.id)}
				{@const Icon = item.icon}
				<li>
					<button type="button" aria-current={view === item.id ? 'page' : undefined} aria-keyshortcuts="Control+{i + 1}" title="Ctrl+{i + 1}" onclick={() => (view = item.id)}>
						<Icon size={17} strokeWidth={1.75} />
						<span>{item.label}</span>
					</button>
				</li>
			{/each}
		</ul>

		<section class="engine" aria-live="polite" aria-label="Engine">
			<p class="engine-state">
				<span class="lamp" class:on={engine.state === 'running'} class:off={engine.state === 'stopped'} aria-hidden="true"></span>
				{engine.state === 'running' ? 'Engine running' : engine.text}
			</p>
			{#if engine.state === 'running'}
				<p class="engine-meta num">{engine.text}</p>
			{:else if engine.state === 'stopped'}
				<p class="engine-meta">Changes are saved and apply when it starts.</p>
			{/if}
		</section>
		{#if app.saveError}
			<p class="save-error" role="alert">Couldn't save settings. {app.saveError}</p>
		{/if}
	</nav>

	<main>
		{#if app.loadError}
			<p class="note err" role="alert">Couldn't read your settings: {app.loadError}</p>
		{:else if !app.config}
			<p class="note">Loading settings…</p>
		{:else}
			{#key view}
				<div class="view" in:fly={{ y: 8, duration: ms(360), easing: expoOut }}>
					{#if view === 'lighting'}
						<LightingView config={app.config} />
					{:else if view === 'keys'}
						<KeysView config={app.config} />
					{:else if view === 'hardware'}
						<HardwareView />
					{:else if view === 'devices'}
						<DevicesView config={app.config} />
					{:else if view === 'settings'}
						<SettingsView config={app.config} />
					{:else}
						<AboutView />
					{/if}
				</div>
			{/key}
		{/if}
	</main>
</div>

<style>
	.app {
		display: grid;
		grid-template-columns: 200px 1fr;
		height: 100vh;
	}
	nav {
		display: flex;
		flex-direction: column;
		gap: 22px;
		padding: 20px 0 16px;
		background: var(--color-raised);
		border-right: 1px solid var(--color-seam);
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 0 20px;
	}
	.brand svg {
		fill: none;
		stroke: var(--color-ink);
		stroke-width: 1.6;
	}
	.wordmark {
		font-size: 15px;
		font-weight: 600;
		font-stretch: 118%;
		letter-spacing: 0.02em;
	}
	ul {
		position: relative;
		list-style: none;
		margin: 0;
		padding: 0 10px;
		display: grid;
		gap: 2px;
	}
	/* The active entry's highlight slides between entries. */
	.marker {
		position: absolute;
		left: 10px;
		right: 10px;
		top: 0;
		height: 38px;
		border-radius: var(--radius);
		background: var(--color-surface-3);
		transform: translateY(calc(var(--i) * 40px));
		transition: transform var(--t-slow) var(--ease);
		pointer-events: none;
	}
	li {
		height: 38px;
	}
	ul button {
		position: relative;
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
		height: 100%;
		padding: 0 12px;
		border: 0;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-3);
		font-size: 14px;
		font-weight: 500;
		text-align: left;
		transition: color var(--t-mid) var(--ease);
	}
	ul button:hover {
		color: var(--color-ink);
	}
	ul button[aria-current='page'] {
		color: var(--color-ink);
	}
	ul button[aria-current='page'] :global(svg) {
		color: var(--color-fac-red);
	}
	ul button:focus-visible {
		outline-offset: -2px;
	}

	.engine {
		margin: auto 10px 0;
		padding: 12px;
		border-radius: var(--radius);
		background: var(--color-surface);
	}
	.engine-state {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 0;
		font-size: 13px;
		font-weight: 500;
	}
	.engine-meta {
		margin: 4px 0 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.45;
	}
	.lamp {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-ink-4);
		transition: background-color var(--t-mid) var(--ease);
	}
	.lamp.on {
		background: #3fb950;
	}
	.lamp.off {
		background: var(--color-fac-yellow);
	}
	.save-error {
		margin: 0 20px;
		color: var(--color-fac-red);
		font-size: 12px;
	}
	main {
		min-width: 0;
		min-height: 0;
		padding: 20px 24px;
		overflow: hidden;
	}
	.view {
		height: 100%;
		min-height: 0;
		container: view / inline-size;
		overflow: auto;
	}
	.note {
		color: var(--color-ink-3);
		font-size: 13px;
	}
	.err {
		color: var(--color-fac-red);
	}
</style>
