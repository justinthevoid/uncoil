<script lang="ts">
	import { onMount } from 'svelte';
	import { fade, fly } from 'svelte/transition';
	import { expoOut } from 'svelte/easing';
	import { Search } from '@lucide/svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import WriteButton from '#lib/components/WriteButton.svelte';
	import Keyboard, { legendFor, type Cap } from '#lib/components/Keyboard.svelte';
	import { daemon, getDesk } from '#lib/api.ts';
	import { pipe, loadDevices, withFeature, errorText } from '#lib/daemon.svelte.ts';
	import { KEY_GROUPS, MODIFIERS, MOUSE_BUTTONS, keyTitle, parseSpec, shortLabel, toSpec, describeFunction, type Mapping } from '#lib/keys.ts';
	import { ms } from '#lib/motion.ts';
	import type { Capabilities, Config, DeskDevice, KeyMapping, Layer, WriteResult } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	let desk = $state<DeskDevice[]>([]);
	let deviceId = $state<string | null>(null);
	let layer = $state<Layer>('hypershift');
	let caps = $state<Capabilities | null>(null);
	let rows = $state<KeyMapping[]>([]);
	let normal = $state<KeyMapping[]>([]);
	let loading = $state(false);
	let loadError = $state<string | null>(null);
	let selectedId = $state<number | null>(null);

	onMount(async () => {
		desk = await getDesk($state.snapshot(config) as Config);
		if (!pipe.loaded) await loadDevices();
	});

	const devices = $derived(withFeature('keymap'));
	$effect(() => {
		if (!deviceId && devices.length) deviceId = devices[0].id;
	});
	const device = $derived(devices.find((d) => d.id === deviceId) ?? null);
	const isMouse = $derived(device?.kind === 'mouse');
	const deviceWord = $derived(isMouse ? 'mouse' : 'keyboard');

	async function load(id: string, l: Layer) {
		loading = true;
		loadError = null;
		try {
			const [c, r, n] = await Promise.all([
				daemon<Capabilities>('capabilities', id),
				daemon<KeyMapping[]>('keymap.dump', id, { layer: l }),
				l === 'normal' ? Promise.resolve(null) : daemon<KeyMapping[]>('keymap.dump', id, { layer: 'normal' })
			]);
			if (id !== deviceId || l !== layer) return;
			caps = c;
			rows = r;
			normal = n ?? r;
			if (!r.some((k) => k.key === selectedId)) selectedId = (r.find((k) => k.name === 'P') ?? r[0])?.key ?? null;
		} catch (e) {
			loadError = errorText(e);
		} finally {
			loading = false;
		}
	}
	$effect(() => {
		if (deviceId) load(deviceId, layer);
	});

	const selected = $derived(rows.find((k) => k.key === selectedId) ?? null);
	const normalById = $derived(new Map(normal.map((k) => [k.key, k.function])));
	/** On the Fn layer: the key does something other than its normal job. */
	const differs = (k: KeyMapping) => layer !== 'normal' && normalById.get(k.key) !== k.function;

	// Keycaps for the drawing: keyed by the layout's shape names (the keymap's LED names).
	const board = $derived(desk.find((d) => d.id === deviceId && d.kind === 'keyboard') ?? null);
	const keyByLed = $derived(new Map((caps?.keys ?? []).filter((k) => k.led).map((k) => [k.led!, k.id])));
	const capMap = $derived.by(() => {
		const m = new Map<string, Cap>();
		const byId = new Map(rows.map((k) => [k.key, k]));
		for (const s of board?.shapes ?? []) {
			if (!s.is_key) continue;
			const id = keyByLed.get(s.name);
			const k = id !== undefined ? byId.get(id) : undefined;
			if (!k) {
				m.set(s.name, { fixed: true });
				continue;
			}
			const d = differs(k);
			m.set(s.name, { sub: d ? shortLabel(k.function) : undefined, accent: d, label: `${legendFor(s.name) || 'Space'}: ${k.description}` });
		}
		return m;
	});
	const selectedLed = $derived((caps?.keys ?? []).find((k) => k.id === selectedId)?.led ?? null);
	function selectLed(led: string) {
		const id = keyByLed.get(led);
		if (id !== undefined) selectedId = id;
	}

	// ---- editor -------------------------------------------------------------------------------------
	let draft = $state<Mapping>({ type: 'off' });
	$effect(() => {
		if (selected) draft = parseSpec(selected.function);
	});
	const draftSpec = $derived(toSpec(draft));
	const dirty = $derived(!!selected && draft.type !== 'other' && draftSpec !== selected.function);

	type Option = { spec: string; label: string; group: string };
	const OPTIONS: Option[] = [
		{ spec: 'key PRINT_SCREEN', label: 'Print screen', group: 'Common' },
		{ spec: 'key MUTE', label: 'Mute', group: 'Common' },
		{ spec: 'key VOLUME_UP', label: 'Volume up', group: 'Common' },
		{ spec: 'key VOLUME_DOWN', label: 'Volume down', group: 'Common' },
		{ spec: 'off', label: 'Do nothing', group: 'Common' },
		...MOUSE_BUTTONS.map((b) => ({ spec: `button ${b.value}`, label: b.label, group: 'Mouse' })),
		...KEY_GROUPS.flatMap((g) => g.keys.map((k) => ({ spec: `key ${k}`, label: describeFunction(`key ${k}`), group: g.label })))
	];
	let query = $state('');
	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		const list = q ? OPTIONS.filter((o) => o.label.toLowerCase().includes(q) || o.spec.toLowerCase().includes(q)) : OPTIONS;
		const groups = new Map<string, Option[]>();
		for (const o of list) groups.set(o.group, [...(groups.get(o.group) ?? []), o]);
		return [...groups];
	});
	/** The chosen base action, ignoring modifiers. */
	const draftBase = $derived(draft.type === 'key' ? `key ${draft.key}` : toSpec(draft));
	function choose(spec: string) {
		const m = parseSpec(spec);
		draft = m.type === 'key' && draft.type === 'key' ? { ...m, mods: draft.mods } : m;
	}
	function toggleMod(spec: string) {
		if (draft.type !== 'key') return;
		draft = { ...draft, mods: draft.mods.includes(spec) ? draft.mods.filter((x) => x !== spec) : [...draft.mods, spec] };
	}

	let busy = $state(false);
	let outcome = $state<{ ok: boolean; text: string } | null>(null);
	$effect(() => {
		void selectedId;
		void layer;
		outcome = null;
		query = '';
	});

	async function write(cmd: 'keymap.set' | 'keymap.reset') {
		if (!selected || !deviceId) return;
		busy = true;
		outcome = null;
		try {
			const args: Record<string, unknown> = { key: `#${selected.key}`, layer, write: true };
			if (cmd === 'keymap.set') args.function = draftSpec;
			const r = await daemon<WriteResult<KeyMapping>>(cmd, deviceId, args);
			rows = rows.map((k) => (k.key === r.after.key ? r.after : k));
			if (layer === 'normal') normal = rows;
			outcome = r.unchanged
				? { ok: true, text: 'It was already set that way.' }
				: r.verified
					? { ok: true, text: `Saved. It now sends ${r.after.description}.` }
					: { ok: false, text: `Saved, but the ${deviceWord} reports ${r.after.description}.` };
		} catch (e) {
			outcome = { ok: false, text: errorText(e) };
		} finally {
			busy = false;
		}
	}

	const layerOptions = [
		{ value: 'normal' as const, label: 'Normal' },
		{ value: 'hypershift' as const, label: 'With Fn held' }
	];
	const keyName = (k: KeyMapping) => {
		const led = (caps?.keys ?? []).find((x) => x.id === k.key)?.led;
		return led ? legendFor(led) || 'Space' : keyTitle(k.name);
	};
</script>

<section class="keys" aria-labelledby="keys-title">
	<header class="head">
		<h1 id="keys-title" class="page-title">Keys</h1>
		<p class="lede">Change what keys and buttons do. Changes are saved in the device itself, so they keep working without uncoil.</p>
	</header>

	{#if !pipe.loaded}
		<p class="state">Connecting to the engine…</p>
	{:else if pipe.unreachable}
		<div class="state panel" in:fade={{ duration: ms(260) }}>
			<p class="section-title">The engine isn't answering</p>
			<p>Key remapping needs the latest uncoild running. Install it with <code>scripts\install-task.ps1</code>, then try again.</p>
			<button type="button" class="btn-quiet" onclick={loadDevices}>Try again</button>
		</div>
	{:else if !devices.length}
		<p class="state">No connected device supports remapping.</p>
	{:else}
		<div class="bar">
			<div class="seg-device">
				<Segmented
					label="Device"
					options={devices.map((d) => ({ value: d.id, label: d.kind === 'keyboard' ? 'Keyboard' : d.kind === 'mouse' ? 'Mouse' : d.name }))}
					value={deviceId ?? ''}
					onchange={(v) => (deviceId = v)}
				/>
			</div>
			<div class="seg-layer">
				<Segmented label="Layer" options={layerOptions} value={layer} onchange={(v) => (layer = v)} />
			</div>
			{#if layer === 'hypershift' && !isMouse}
				<p class="hint"><span class="swatch" aria-hidden="true"></span>Tinted keys do something different while Fn is held</p>
			{/if}
		</div>

		<div class="body">
			<div class="map" aria-busy={loading}>
				{#if loadError}
					<p class="outcome bad" role="alert">{loadError}</p>
				{:else if board && caps}
					<div class="kb"><Keyboard device={board} caps={capMap} selected={selectedLed} onselect={selectLed} /></div>
				{:else}
					<div class="buttons" role="listbox" aria-label="Mouse buttons">
						{#each rows as k (k.key)}
							<button type="button" role="option" aria-selected={k.key === selectedId} class:accent={differs(k)} onclick={() => (selectedId = k.key)}>
								<span class="bname">{keyTitle(k.name)}</span>
								<span class="bdesc">{k.description}</span>
							</button>
						{/each}
					</div>
				{/if}
			</div>

			<aside class="inspector" aria-label="Selected key">
				{#if selected}
					{#key `${selected.key}-${layer}`}
						<div class="insp" in:fly={{ x: 8, duration: ms(280), easing: expoOut }}>
							<div class="col-info">
							<h2 class="kname">{layer === 'hypershift' ? 'Fn + ' : ''}{keyName(selected)}</h2>
							<p class="now"><span class="label">Currently</span>{selected.description}</p>
							{#if dirty}
								<p class="will" in:fade={{ duration: ms(160) }}><span class="label">Will send</span>{describeFunction(draftSpec)}</p>
							{/if}
							</div>

							<div class="picker">
								<label class="label" for="key-search">Change to</label>
								<div class="search">
									<Search size={14} />
									<input id="key-search" class="input" type="search" placeholder="Search keys and actions" autocomplete="off" bind:value={query} />
								</div>
								<div class="options" role="listbox" aria-label="Actions">
									{#each filtered as [group, opts] (group)}
										<p class="group">{group}</p>
										{#each opts as o (o.spec)}
											<button type="button" role="option" aria-selected={o.spec === draftBase} onclick={() => choose(o.spec)}>{o.label}</button>
										{/each}
									{:else}
										<p class="empty">Nothing matches “{query}”.</p>
									{/each}
								</div>
							</div>

							<div class="col-act">
							{#if draft.type === 'key'}
								<p class="label">Hold together with</p>
								<div class="mods" role="group" aria-label="Hold together with">
									{#each MODIFIERS as m (m.spec)}
										<button type="button" class="mod" aria-pressed={draft.mods.includes(m.spec)} onclick={() => toggleMod(m.spec)}>{m.label}</button>
									{/each}
								</div>
							{:else if draft.type === 'other'}
								<p class="note">This key uses a kind of action uncoil can't edit yet (macro, DPI, profile or a Synapse-only key). Pick something above to replace it.</p>
							{/if}

							<div class="actions">
								<WriteButton
									label="Save to {deviceWord}"
									warning="This is saved in the {deviceWord}'s own memory, so it works even without uncoil. Restore original puts back what was there before."
									disabled={!dirty}
									{busy}
									onconfirm={() => write('keymap.set')}
								/>
								<WriteButton
									quiet
									label="Restore original"
									warning="Puts back what this key did before uncoil changed it (or the factory setting)."
									{busy}
									onconfirm={() => write('keymap.reset')}
								/>
							</div>
							{#if outcome}
								<p class="outcome" class:bad={!outcome.ok} role="status" in:fade={{ duration: ms(200) }}>{outcome.text}</p>
							{/if}
							</div>
						</div>
					{/key}
				{:else}
					<p class="note">Pick a key to change it.</p>
				{/if}
			</aside>
		</div>
	{/if}
</section>

<style>
	.keys {
		display: grid;
		grid-template-rows: auto auto auto;
		gap: 16px;
		align-content: start;
	}
	.lede {
		margin: 6px 0 0;
		color: var(--color-ink-3);
		font-size: 13px;
		max-width: 70ch;
	}
	.state {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 13px;
	}
	.panel {
		display: grid;
		gap: 10px;
		justify-items: start;
		padding: 20px;
		border-radius: var(--radius-lg);
		background: var(--color-raised);
		border: 1px solid var(--color-seam);
	}
	.panel p {
		margin: 0;
		max-width: 60ch;
		color: var(--color-ink-2);
	}
	code {
		font-family: ui-monospace, 'Cascadia Mono', Consolas, monospace;
		font-size: 12px;
		color: var(--color-ink);
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 12px;
		flex-wrap: wrap;
	}
	.seg-device {
		width: 220px;
	}
	.seg-layer {
		width: 260px;
	}
	.hint {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 0 0 0 auto;
		color: var(--color-ink-3);
		font-size: 12px;
	}
	.swatch {
		width: 12px;
		height: 12px;
		border-radius: 3px;
		background: #2a1a19;
		box-shadow: inset 0 0 0 1px #4a2a28;
	}
	.body {
		display: grid;
		grid-template-rows: auto auto;
		gap: 16px;
		min-height: 0;
	}
	.map {
		display: grid;
		justify-items: center;
		align-content: center;
		min-height: 0;
		padding: 16px 24px;
		border-radius: var(--radius-lg);
		background: radial-gradient(ellipse at 50% 40%, #151515, #0d0d0d 75%);
		border: 1px solid var(--color-seam);
		overflow: auto;
		transition: opacity var(--t-mid) var(--ease);
	}
	.map[aria-busy='true'] {
		opacity: 0.6;
	}
	.kb {
		width: 100%;
		max-width: 780px;
	}
	.buttons {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 6px;
		width: 100%;
		align-self: start;
	}
	.buttons button {
		display: grid;
		grid-template-columns: 160px 1fr;
		align-items: center;
		gap: 12px;
		height: 42px;
		padding: 0 14px;
		border: 0;
		border-radius: var(--radius);
		background: var(--color-surface);
		color: var(--color-ink);
		text-align: left;
		font-size: 13px;
		transition: background-color var(--t-mid) var(--ease);
	}
	.buttons button:hover {
		background: var(--color-surface-2);
	}
	.buttons button[aria-selected='true'] {
		background: var(--color-surface-3);
		box-shadow: inset 0 0 0 2px var(--color-ink);
	}
	.buttons button.accent .bdesc {
		color: var(--color-fac-red);
	}
	.bname {
		font-weight: 500;
	}
	.bdesc {
		color: var(--color-ink-3);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.inspector {
		min-height: 0;
		min-width: 0;
		padding: 18px;
		border-radius: var(--radius-lg);
		background: var(--color-raised);
		border: 1px solid var(--color-seam);
		overflow: hidden auto;
	}
	.insp {
		display: grid;
		grid-template-columns: minmax(0, 0.8fr) minmax(0, 1fr) minmax(0, 0.9fr);
		gap: 24px;
		align-items: start;
	}
	.col-info,
	.col-act {
		display: grid;
		gap: 12px;
		align-content: start;
	}
	.kname {
		margin: 0;
		font-size: 22px;
		font-weight: 600;
		font-stretch: 112%;
	}
	.now,
	.will {
		display: grid;
		gap: 2px;
		margin: 0;
		font-size: 14px;
	}
	.picker {
		display: grid;
		gap: 8px;
	}
	.search {
		position: relative;
		display: grid;
		align-items: center;
		color: var(--color-ink-3);
	}
	.search :global(svg) {
		position: absolute;
		left: 11px;
		pointer-events: none;
	}
	.search input {
		padding-left: 32px;
		width: 100%;
	}
	.options {
		display: grid;
		max-height: 170px;
		overflow: auto;
		padding: 4px;
		border-radius: var(--radius);
		background: var(--color-ground);
		border: 1px solid var(--color-seam);
	}
	.group {
		margin: 8px 8px 4px;
		color: var(--color-ink-4);
		font-size: 11px;
		font-weight: 600;
	}
	.group:first-child {
		margin-top: 4px;
	}
	.options button {
		height: 30px;
		padding: 0 10px;
		border: 0;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--color-ink-2);
		text-align: left;
		font-size: 13px;
	}
	.options button:hover {
		background: var(--color-surface-2);
		color: var(--color-ink);
	}
	.options button[aria-selected='true'] {
		background: var(--color-surface-3);
		color: var(--color-ink);
		font-weight: 500;
	}
	.empty {
		margin: 8px;
		color: var(--color-ink-3);
		font-size: 13px;
	}
	.mods {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 6px;
	}
	.mod {
		height: 30px;
		border: 1px solid var(--color-seam-2);
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-3);
		font-size: 12px;
		font-weight: 500;
		transition:
			color var(--t-mid) var(--ease),
			background-color var(--t-mid) var(--ease),
			border-color var(--t-mid) var(--ease);
	}
	.mod:hover {
		color: var(--color-ink);
	}
	.mod[aria-pressed='true'] {
		border-color: var(--color-ink);
		background: var(--color-ink);
		color: var(--color-ground);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.actions :global(.write:has(.confirm)) {
		flex-basis: 100%;
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
	}
	@container view (max-width: 820px) {
		.insp {
			grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		}
		.col-act {
			grid-column: 1 / -1;
		}
		.hint {
			margin-left: 0;
		}
	}
</style>
