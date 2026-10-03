<script lang="ts">
	import { onMount } from 'svelte';
	import { fly } from 'svelte/transition';
	import { expoOut } from 'svelte/easing';
	import { app, loadConfig, pollStatus, scheduleSave } from '#lib/state.svelte.ts';
	import { ms } from '#lib/motion.ts';
	import LightingView from '#lib/views/LightingView.svelte';
	import DevicesView from '#lib/views/DevicesView.svelte';
	import DisplayView from '#lib/views/DisplayView.svelte';
	import AboutView from '#lib/views/AboutView.svelte';

	type View = 'lighting' | 'devices' | 'display' | 'about';
	const nav: { id: View; label: string; code: string }[] = [
		{ id: 'lighting', label: 'Lighting', code: '01' },
		{ id: 'devices', label: 'Devices', code: '02' },
		{ id: 'display', label: 'Display', code: '03' },
		{ id: 'about', label: 'About', code: '04' }
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
	const kb = (b: number) => `${Math.round(b / 1024)} KB`;
	const engine = $derived.by(() => {
		if (!app.statusKnown) return { state: 'checking', rows: [] as [string, string][] };
		const s = app.status;
		if (!s) return { state: 'stopped', rows: [] as [string, string][] };
		const rows: [string, string][] = [];
		if (s.memory_bytes) rows.push(['Memory', mb(s.memory_bytes)]);
		if (s.memory_bytes) rows.push(['CPU', `${s.cpu_percent.toFixed(1)} %`]);
		if (s.exe_bytes) rows.push(['Size', kb(s.exe_bytes)]);
		rows.push(['Devices', `${s.devices.length} live`]);
		rows.push(['Display', s.display]);
		return { state: 'running', rows };
	});

	// one keyboard shortcut per section: Ctrl+1..4
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
			{#each nav as item (item.id)}
				<li>
					<button type="button" aria-current={view === item.id ? 'page' : undefined} aria-keyshortcuts="Control+{item.code.slice(1)}" onclick={() => (view = item.id)}>
						<span class="code num">{item.code}</span>
						<span class="caps label">{item.label}</span>
					</button>
				</li>
			{/each}
		</ul>

		<section class="engine" aria-live="polite" aria-label="Engine">
			<h2 class="caps-sm">
				<span class="lamp" class:on={engine.state === 'running'} aria-hidden="true"></span>
				uncoild {engine.state === 'running' ? 'running' : engine.state === 'stopped' ? 'not running' : '…'}
			</h2>
			{#if engine.state === 'running'}
				<dl>
					{#each engine.rows as [k, v] (k)}
						<div><dt class="caps-sm">{k}</dt><dd class="num">{v}</dd></div>
					{/each}
				</dl>
			{:else if engine.state === 'stopped'}
				<p class="stopped">Settings are saved and apply when the engine starts.</p>
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
				<div class="view" in:fly={{ y: 10, duration: ms(420), easing: expoOut }}>
					{#if view === 'lighting'}
						<LightingView config={app.config} />
					{:else if view === 'devices'}
						<DevicesView config={app.config} />
					{:else if view === 'display'}
						<DisplayView config={app.config} />
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
		grid-template-columns: 196px 1fr;
		height: 100vh;
	}
	nav {
		display: flex;
		flex-direction: column;
		gap: 28px;
		padding: 22px 0 18px;
		border-right: var(--hair);
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
		stroke-width: 1.4;
	}
	.wordmark {
		font-size: 13px;
		font-weight: 500;
		font-stretch: 125%;
		letter-spacing: 0.3em;
		text-transform: lowercase;
	}
	ul {
		position: relative;
		list-style: none;
		margin: 0;
		padding: 0 12px;
		display: grid;
		gap: 4px;
	}
	/* The active entry is boxed like a catalog index; the box slides between entries. */
	.marker {
		position: absolute;
		left: 12px;
		right: 12px;
		top: 0;
		height: 40px;
		border: var(--hair-ink);
		transform: translateY(calc(var(--i) * 44px));
		transition: transform var(--t-slow) var(--ease);
		pointer-events: none;
	}
	li {
		height: 40px;
	}
	ul button {
		display: flex;
		align-items: center;
		gap: 14px;
		width: 100%;
		height: 100%;
		padding: 0 12px;
		border: 0;
		background: none;
		color: var(--color-ink-3);
		text-align: left;
		transition: color var(--t-mid) var(--ease);
	}
	ul button:hover {
		color: var(--color-ink-2);
	}
	ul button[aria-current='page'] {
		color: var(--color-ink);
	}
	.code {
		font-size: 12px;
		font-stretch: 112%;
		letter-spacing: 0.08em;
		transition: color var(--t-mid) var(--ease);
	}
	ul button[aria-current='page'] .code {
		color: var(--color-fac-red);
	}
	ul button:focus-visible {
		outline-offset: -3px;
	}

	.engine {
		margin: auto 12px 0;
		padding: 14px 12px 12px;
		border-top: var(--hair);
	}
	.engine h2 {
		display: flex;
		align-items: center;
		gap: 9px;
		margin: 0 0 12px;
		color: var(--color-ink);
		font-weight: 500;
	}
	.lamp {
		width: 7px;
		height: 7px;
		background: var(--color-seam-2);
		transition: background-color var(--t-mid) var(--ease);
	}
	.lamp.on {
		background: var(--color-fac-red);
	}
	dl {
		margin: 0;
		display: grid;
		gap: 7px;
	}
	dl div {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
	}
	dt {
		color: var(--color-ink-3);
	}
	dd {
		margin: 0;
		font-size: 12px;
		font-stretch: 108%;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}
	.stopped {
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.45;
		margin: 0;
	}
	.save-error {
		margin: 0 22px;
		color: var(--color-fac-red);
		font-size: 12px;
	}
	main {
		min-width: 0;
		min-height: 0;
		padding: 18px;
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
