<script lang="ts">
	import { onMount, type Component } from 'svelte';
	import { fade } from 'svelte/transition';
	import { CircleDot, Info, Layers, Keyboard as KeyboardIcon, LayoutGrid, Lightbulb, Monitor, Mouse, Palette, RectangleHorizontal, Settings as SettingsIcon } from '@lucide/svelte';
	import { app, loadConfig, pollStatus, scheduleSave } from '#lib/state.svelte.ts';
	import { daemon, getDesk } from '#lib/api.ts';
	import { pipe, loadDevices } from '#lib/daemon.svelte.ts';
	import { ms } from '#lib/motion.ts';
	import LightingView from '#lib/views/LightingView.svelte';
	import StudioView from '#lib/views/StudioView.svelte';
	import DevicesView from '#lib/views/DevicesView.svelte';
	import KeysView from '#lib/views/KeysView.svelte';
	import DialScreenView from '#lib/views/DialScreenView.svelte';
	import OnboardEffectsView from '#lib/views/OnboardEffectsView.svelte';
	import DeviceInfoView from '#lib/views/DeviceInfoView.svelte';
	import SettingsView from '#lib/views/SettingsView.svelte';
	import AboutView from '#lib/views/AboutView.svelte';
	import type { Config, DeskDevice, DeviceKind, ProfileInfo } from '#lib/types.ts';

	type Icon = Component<{ size?: number; strokeWidth?: number }>;
	type Feature = 'lighting' | 'studio' | 'devices' | 'keys' | 'dial' | 'effects' | 'info' | 'settings' | 'about';

	const shortName = (n: string) => n.replace(/^Razer /, '').replace(/ Chroma Extended$/, ' Chroma');

	let desk = $state<DeskDevice[]>([]);
	let tab = $state<string>('desk');
	let feature = $state<Feature>('lighting');
	const remembered = new Map<string, Feature>();

	const kindIcon: Record<DeviceKind, Icon> = { keyboard: KeyboardIcon, mouse: Mouse, mousemat: RectangleHorizontal, headset: Info, other: Info };
	const FEATURES: Record<string, { id: Feature; label: string; icon: Icon }[]> = {
		desk: [
			{ id: 'lighting', label: 'Lighting', icon: Lightbulb },
			{ id: 'studio', label: 'Studio', icon: Layers },
			{ id: 'devices', label: 'Devices', icon: LayoutGrid }
		],
		keyboard: [
			{ id: 'keys', label: 'Keys', icon: KeyboardIcon },
			{ id: 'dial', label: 'Dial & screen', icon: CircleDot },
			{ id: 'effects', label: 'Onboard effects', icon: Palette },
			{ id: 'info', label: 'Device info', icon: Info }
		],
		mouse: [
			{ id: 'keys', label: 'Buttons', icon: Mouse },
			{ id: 'effects', label: 'Onboard effects', icon: Palette },
			{ id: 'info', label: 'Device info', icon: Info }
		],
		mousemat: [
			{ id: 'effects', label: 'Onboard effects', icon: Palette },
			{ id: 'info', label: 'Device info', icon: Info }
		],
		settings: [
			{ id: 'settings', label: 'Display & RGB', icon: Monitor },
			{ id: 'about', label: 'About', icon: Info }
		]
	};

	const device = $derived(desk.find((d) => d.id === tab) ?? null);
	const features = $derived(tab === 'desk' ? FEATURES.desk : tab === 'settings' ? FEATURES.settings : (FEATURES[device?.kind ?? ''] ?? FEATURES.mousemat));
	const railTitle = $derived(tab === 'desk' ? 'Whole desk' : tab === 'settings' ? 'Settings' : shortName(device?.name ?? ''));

	function openTab(id: string) {
		remembered.set(tab, feature);
		tab = id;
		const list = id === 'desk' ? FEATURES.desk : id === 'settings' ? FEATURES.settings : (FEATURES[desk.find((d) => d.id === id)?.kind ?? ''] ?? FEATURES.mousemat);
		const last = remembered.get(id);
		feature = last && list.some((f) => f.id === last) ? last : list[0].id;
	}

	const live = (id: string) => !!app.status?.devices.some((d) => d.id === id);

	// The selected device's onboard profiles, for the rail note.
	let profile = $state<{ id: string; info: ProfileInfo } | null>(null);
	$effect(() => {
		const id = device?.id;
		if (!id || !pipe.devices.find((d) => d.id === id)?.features.includes('profiles')) return;
		daemon<ProfileInfo>('profile.list', id)
			.then((info) => (profile = { id, info }))
			.catch(() => (profile = null));
	});
	const profileText = $derived(profile && profile.id === device?.id ? `Profile ${profile.info.active ?? 1} of ${profile.info.max} slots (${profile.info.count} in use).` : '');

	onMount(() => {
		loadDevices();
		loadConfig().then(async () => {
			if (app.config) desk = await getDesk($state.snapshot(app.config) as Config);
		});
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
		if (!app.statusKnown) return { state: 'checking', text: 'Checking the engine…' };
		const s = app.status;
		if (!s) return { state: 'stopped', text: 'Engine not running' };
		const parts = ['Engine running'];
		if (s.memory_bytes) parts.push(mb(s.memory_bytes), `${s.cpu_percent.toFixed(1)}% CPU`);
		return { state: 'running', text: parts.join(' · ') };
	});

	// Ctrl+1..9 switch the top tabs; Ctrl+, opens settings.
	function onkeydown(e: KeyboardEvent) {
		if (!e.ctrlKey || e.altKey || e.metaKey) return;
		if (e.key === ',') {
			openTab('settings');
			e.preventDefault();
			return;
		}
		const n = Number(e.key);
		const tabs = ['desk', ...desk.map((d) => d.id)];
		if (n >= 1 && n <= tabs.length) {
			openTab(tabs[n - 1]);
			e.preventDefault();
		}
	}
</script>

<svelte:window {onkeydown} />

<div class="app">
	<header class="top">
		<div class="brand">
			<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d="M12 12a2 2 0 1 1 2-2 4 4 0 1 1-4-4 6 6 0 1 1-6 6 8 8 0 1 1 8 8" /></svg>
			<span>uncoil</span>
		</div>
		<nav class="tabs" aria-label="Desk and devices">
			<button type="button" class="tab" aria-current={tab === 'desk' ? 'page' : undefined} onclick={() => openTab('desk')}>
				<LayoutGrid size={16} strokeWidth={1.75} />Desk
			</button>
			{#each desk as d (d.id)}
				{@const Icon = kindIcon[d.kind]}
				<button type="button" class="tab" aria-current={tab === d.id ? 'page' : undefined} onclick={() => openTab(d.id)}>
					<Icon size={16} strokeWidth={1.75} />{shortName(d.name)}
					<span class="dot" class:on={live(d.id)} class:unknown={!app.status} aria-label={!app.status ? 'Engine not running' : live(d.id) ? 'Connected' : 'Not connected'}></span>
					{#if app.status && !live(d.id)}<span class="away">Not connected</span>{/if}
				</button>
			{/each}
		</nav>
		<div class="right">
			<span class="engine" aria-live="polite"
				><span class="dot" class:on={engine.state === 'running'} class:warn={engine.state === 'stopped'}></span><span class="long">{engine.text}</span><span class="short"
					>{engine.state === 'running' ? 'Running' : engine.state === 'stopped' ? 'Engine off' : '…'}</span
				></span
			>
			<button type="button" class="iconbtn" aria-label="Settings" aria-pressed={tab === 'settings'} title="Settings (Ctrl+,)" onclick={() => openTab('settings')}>
				<SettingsIcon size={17} strokeWidth={1.75} />
			</button>
		</div>
	</header>

	<nav class="rail" aria-label="{railTitle} features">
		<p class="rail-title">{railTitle}</p>
		{#each features as f (f.id)}
			{@const Icon = f.icon}
			<button type="button" aria-current={feature === f.id ? 'page' : undefined} onclick={() => (feature = f.id)}>
				<Icon size={16} strokeWidth={1.75} />{f.label}
			</button>
		{/each}
		{#if app.saveError}
			<p class="save-error" role="alert">Couldn't save settings. {app.saveError}</p>
		{/if}
		{#if engine.state === 'stopped'}
			<p class="rail-note">The engine isn't running. Lighting changes are saved and apply when it starts.</p>
		{:else if device}
			<p class="rail-note">{#if profileText}<b>{profileText}</b><br />{/if}Changes on this {device.kind === 'mousemat' ? 'mat' : device.kind}'s pages are saved in the device itself, so they keep working without uncoil.</p>
		{/if}
	</nav>

	<main>
		{#if app.loadError}
			<p class="note err" role="alert">Couldn't read your settings: {app.loadError}</p>
		{:else if !app.config}
			<p class="note">Loading settings…</p>
		{:else}
			{#key `${tab}/${feature}`}
				<div class="view" in:fade={{ duration: ms(160) }}>
					{#if feature === 'lighting'}
						<LightingView config={app.config} onstudio={() => (feature = 'studio')} />
					{:else if feature === 'studio'}
						<StudioView config={app.config} />
					{:else if feature === 'devices'}
						<DevicesView config={app.config} onopen={openTab} />
					{:else if feature === 'keys' && device}
						<KeysView config={app.config} deviceId={device.id} />
					{:else if feature === 'dial' && device}
						<DialScreenView deviceId={device.id} />
					{:else if feature === 'effects' && device}
						<OnboardEffectsView deviceId={device.id} />
					{:else if feature === 'info' && device}
						<DeviceInfoView {device} />
					{:else if feature === 'settings'}
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
		grid-template-rows: 52px 1fr;
		grid-template-columns: 208px 1fr;
		grid-template-areas: 'top top' 'rail main';
		height: 100vh;
	}
	.top {
		grid-area: top;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 12px 0 16px;
		border-bottom: var(--hair);
		background: var(--color-raised);
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 180px;
		font-family: var(--font-display);
		font-size: 15px;
		font-weight: 650;
	}
	.brand svg {
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
	}
	.tabs {
		display: flex;
		gap: 2px;
		min-width: 0;
	}
	.tab {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 34px;
		padding: 0 12px;
		border: 0;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-3);
		font-weight: 500;
		white-space: nowrap;
		transition:
			background-color var(--t-mid) var(--ease),
			color var(--t-mid) var(--ease);
	}
	.tab:hover {
		color: var(--color-ink);
	}
	.tab[aria-current='page'] {
		background: var(--color-surface-2);
		color: var(--color-ink);
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--color-ink-4);
		flex: none;
	}
	.dot.on {
		background: var(--color-ok);
	}
	.dot.warn {
		background: var(--color-warn);
	}
	.dot.unknown {
		background: transparent;
		box-shadow: inset 0 0 0 1px var(--color-ink-4);
	}
	.away {
		color: var(--color-ink-4);
		font-size: 12px;
		font-weight: 400;
	}
	.right {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-left: auto;
	}
	.engine {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--color-ink-3);
		font-size: 12px;
		white-space: nowrap;
	}
	.iconbtn {
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
		border: 0;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-3);
	}
	.iconbtn:hover,
	.iconbtn[aria-pressed='true'] {
		background: var(--color-surface-2);
		color: var(--color-ink);
	}
	.rail {
		grid-area: rail;
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 14px 10px;
		border-right: var(--hair);
		background: var(--color-raised);
	}
	.rail-title {
		margin: 2px 10px 8px;
		color: var(--color-ink-3);
		font-size: 12px;
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.rail button {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 34px;
		padding: 0 10px;
		border: 0;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-2);
		font-weight: 500;
		text-align: left;
		transition:
			background-color var(--t-mid) var(--ease),
			color var(--t-mid) var(--ease);
	}
	.rail button:hover {
		color: var(--color-ink);
	}
	.rail button[aria-current='page'] {
		background: var(--color-surface-2);
		color: var(--color-ink);
	}
	.rail-note,
	.save-error {
		margin: auto 6px 0;
		padding: 10px;
		border-radius: var(--radius);
		background: var(--color-surface);
		border: var(--hair);
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.45;
	}
	.save-error {
		color: var(--color-warn);
	}
	.rail-note b {
		color: var(--color-ink-2);
		font-weight: 600;
	}
	main {
		grid-area: main;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
	}
	.view {
		height: 100%;
		min-height: 0;
		container: view / inline-size;
		overflow: auto;
	}
	.note {
		margin: 24px;
		color: var(--color-ink-3);
	}
	.err {
		color: var(--color-warn);
	}
	.short {
		display: none;
	}
	@media (max-width: 1180px) {
		.away,
		.long {
			display: none;
		}
		.short {
			display: inline;
		}
	}
</style>
