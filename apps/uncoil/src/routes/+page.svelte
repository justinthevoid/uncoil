<script lang="ts">
	import { onMount, type Component } from 'svelte';
	import { fade } from 'svelte/transition';
	import { BatteryMedium, Bell, CircleDot, X, Gauge, Info, Layers, Keyboard as KeyboardIcon, LayoutGrid, Lightbulb, Monitor, Mouse, Palette, RectangleHorizontal, Settings as SettingsIcon } from '@lucide/svelte';
	import { app, deskSources, loadConfig, pollStatus, scheduleSave } from '#lib/state.svelte.ts';
	import { daemon, getDesk, onConfigChanged } from '#lib/api.ts';
	import { pipe, loadDevices, shortName } from '#lib/daemon.svelte.ts';
	import { PAGES, type PageId } from '#lib/pages.ts';
	import { ms } from '#lib/motion.ts';
	import LightingView from '#lib/views/LightingView.svelte';
	import StudioView from '#lib/views/StudioView.svelte';
	import DevicesView from '#lib/views/DevicesView.svelte';
	import KeysView from '#lib/views/KeysView.svelte';
	import DialScreenView from '#lib/views/DialScreenView.svelte';
	import OnboardEffectsView from '#lib/views/OnboardEffectsView.svelte';
	import DeviceInfoView from '#lib/views/DeviceInfoView.svelte';
	import PerformanceView from '#lib/views/PerformanceView.svelte';
	import PowerView from '#lib/views/PowerView.svelte';
	import { EXPERIMENTAL, EXPERIMENTAL_TEXT } from '#lib/checks.ts';
	import SettingsView from '#lib/views/SettingsView.svelte';
	import AppSettingsView from '#lib/views/AppSettingsView.svelte';
	import AboutView from '#lib/views/AboutView.svelte';
	import type { Config, DeskDevice, DeviceInfo, DeviceKind, Feature, ProfileInfo } from '#lib/types.ts';

	type Icon = Component<{ size?: number; strokeWidth?: number }>;
	/** A page the rail can open (Keys and Buttons are one page, named for the device). */
	type RailPage = Exclude<PageId, 'buttons'>;
	type Entry = { id: RailPage; label: string; icon: Icon };
	/** A device tab: on the desk (has a layout), or only known from the engine (no lighting, e.g. most mice without RGB). */
	type Tab = { id: string; name: string; kind: DeviceKind; desk: DeskDevice | null; info: DeviceInfo | null };

	let desk = $state<DeskDevice[]>([]);
	let tab = $state<string>('desk');
	let feature = $state<RailPage>('lighting');
	const remembered = new Map<string, RailPage>();

	const kindIcon: Record<DeviceKind, Icon> = { keyboard: KeyboardIcon, mouse: Mouse, mousemat: RectangleHorizontal, headset: Info, other: Info };
	const entry = (id: RailPage, icon: Icon, name: PageId = id): Entry => ({ id, label: PAGES[name].label, icon });
	const DESK: Entry[] = [entry('lighting', Lightbulb), entry('studio', Layers), entry('devices', LayoutGrid)];
	const SETTINGS: Entry[] = [entry('settings', Monitor), entry('app', Bell), entry('about', Info)];
	/** What a device can do before the engine has said (or while it's unplugged): today's defaults per kind. */
	const DEFAULT_FEATURES: Record<DeviceKind, Feature[]> = {
		keyboard: ['keymap', 'dial', 'hw_effects'],
		mouse: ['keymap', 'hw_effects'],
		mousemat: ['hw_effects'],
		headset: [],
		other: []
	};

	/** The rail for a device, from what it supports. */
	function deviceEntries(t: Tab): Entry[] {
		const f = t.info?.features ?? DEFAULT_FEATURES[t.kind];
		const has = (x: Feature) => f.includes(x);
		const list: Entry[] = [];
		if (has('keymap')) list.push(t.kind === 'mouse' ? entry('keys', Mouse, 'buttons') : entry('keys', KeyboardIcon));
		if (has('dpi') || has('poll_rate') || has('scroll')) list.push(entry('performance', Gauge));
		if (has('power')) list.push(entry('power', BatteryMedium));
		if (has('dial') || has('oled')) list.push(entry('dial', CircleDot));
		if (has('hw_effects')) list.push(entry('effects', Palette));
		list.push(entry('info', Info));
		return list;
	}

	// Desk devices first (their order), then connected devices that have no place on the desk.
	const tabs = $derived.by((): Tab[] => {
		// Devices driven through OpenRGB sit on the desk but have no pages of their own (Devices lists them).
		const list: Tab[] = desk.filter((d) => !d.id.startsWith('openrgb:')).map((d) => ({ id: d.id, name: d.name, kind: d.kind, desk: d, info: pipe.devices.find((p) => p.id === d.id) ?? null }));
		for (const p of pipe.devices) if (!desk.some((d) => d.id === p.id)) list.push({ id: p.id, name: p.name, kind: p.kind, desk: null, info: p });
		return list;
	});
	const entriesFor = (id: string) => {
		if (id === 'desk') return DESK;
		if (id === 'settings') return SETTINGS;
		const t = tabs.find((x) => x.id === id);
		return t ? deviceEntries(t) : [entry('info', Info)];
	};

	const device = $derived(tabs.find((d) => d.id === tab) ?? null);
	const features = $derived(entriesFor(tab));
	const experimental = $derived(device?.info?.support === 'experimental');
	const railTitle = $derived(tab === 'desk' ? 'Whole desk' : tab === 'settings' ? 'Settings' : shortName(device?.name ?? ''));

	function openTab(id: string) {
		remembered.set(tab, feature);
		tab = id;
		const list = entriesFor(id);
		const last = remembered.get(id);
		feature = last && list.some((f) => f.id === last) ? last : list[0].id;
	}
	// The engine's device list can arrive after a tab is open; keep the page on something the rail offers.
	$effect(() => {
		if (!features.some((f) => f.id === feature)) feature = features[0].id;
	});

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

	// The desk (and so the device tabs) follows what is connected: experimental devices with a layout join it.
	const deskKey = $derived(deskSources().key);
	$effect(() => {
		deskKey;
		const config = app.config;
		const src = deskSources();
		if (config) getDesk($state.snapshot(config) as Config, src.connected, src.external).then((d) => (desk = d));
	});

	onMount(() => {
		loadDevices();
		loadConfig();
		pollStatus();
		const id = setInterval(pollStatus, 2000);
		// The tray's effect menu edits config.json; take its version so this window doesn't save over it.
		let unlisten: (() => void) | undefined;
		onConfigChanged(() => {
			saved = '';
			loadConfig();
		}).then((u) => (unlisten = u));
		return () => {
			clearInterval(id);
			unlisten?.();
		};
	});

	// Other programs driving the same devices (Synapse, SignalRGB…): a notice until dismissed for this session.
	let dismissed = $state<string[]>([]);
	const conflicts = $derived((app.status?.conflicts ?? []).filter((c) => !dismissed.includes(c.app)));

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
		const ids = ['desk', ...tabs.map((d) => d.id)];
		if (n >= 1 && n <= ids.length) {
			openTab(ids[n - 1]);
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
			{#each tabs as d (d.id)}
				{@const Icon = kindIcon[d.kind]}
				{@const exp = d.info?.support === 'experimental'}
				<button type="button" class="tab" aria-current={tab === d.id ? 'page' : undefined} title={exp ? `${EXPERIMENTAL}: ${EXPERIMENTAL_TEXT}` : undefined} onclick={() => openTab(d.id)}>
					<Icon size={16} strokeWidth={1.75} /><span class="tab-name">{shortName(d.name)}</span>
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

	{#if conflicts.length}
		<div class="conflicts" role="status" aria-label="Other programs driving your devices">
			{#each conflicts as c (c.app)}
				<p class="conflict">
					<span class="dot warn" aria-hidden="true"></span><span>{c.detail}</span>
					<button type="button" class="dismiss" aria-label="Dismiss the notice about {c.app}" title="Dismiss until uncoil restarts" onclick={() => (dismissed = [...dismissed, c.app])}><X size={15} strokeWidth={1.75} /></button>
				</p>
			{/each}
		</div>
	{/if}

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
		{:else if device && experimental}
			<p class="rail-note"><b>{EXPERIMENTAL}.</b> {EXPERIMENTAL_TEXT} Device info has the checks and a link to tell us how it went.</p>
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
					{:else if feature === 'performance' && device}
						<PerformanceView deviceId={device.id} />
					{:else if feature === 'power' && device}
						<PowerView deviceId={device.id} />
					{:else if feature === 'dial' && device}
						<DialScreenView deviceId={device.id} />
					{:else if feature === 'effects' && device}
						<OnboardEffectsView deviceId={device.id} />
					{:else if feature === 'info' && device}
						<DeviceInfoView device={{ id: device.id, name: device.name, kind: device.kind }} desk={device.desk} />
					{:else if feature === 'settings'}
						<SettingsView config={app.config} />
					{:else if feature === 'app'}
						<AppSettingsView />
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
		grid-template-rows: 52px auto 1fr;
		grid-template-columns: 208px 1fr;
		grid-template-areas: 'top top' 'notice notice' 'rail main';
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
		overflow: hidden;
	}
	.tab-name {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
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
		min-width: 0;
		flex: 0 1 auto;
		transition:
			background-color var(--t-mid) var(--ease),
			color var(--t-mid) var(--ease);
	}
	.tab :global(svg),
	.tab .dot {
		flex: none;
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
	.conflicts {
		grid-area: notice;
		display: grid;
		border-bottom: var(--hair);
		background: var(--color-surface);
	}
	.conflict {
		display: flex;
		align-items: center;
		gap: 10px;
		margin: 0;
		padding: 6px 12px 6px 18px;
		color: var(--color-ink-2);
		font-size: 13px;
		line-height: 1.45;
	}
	.conflict + .conflict {
		border-top: var(--hair);
	}
	.conflict > span:nth-child(2) {
		margin-right: auto;
	}
	.dismiss {
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		flex: none;
		border: 0;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-3);
	}
	.dismiss:hover {
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
		.brand {
			width: auto;
			margin-right: 14px;
		}
	}
</style>
