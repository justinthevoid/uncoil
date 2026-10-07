<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { Search } from '@lucide/svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import Segmented from '#lib/components/Segmented.svelte';
	import WriteButton from '#lib/components/WriteButton.svelte';
	import PipeUnavailable from '#lib/components/PipeUnavailable.svelte';
	import Keyboard, { legendFor, type Cap } from '#lib/components/Keyboard.svelte';
	import MouseDiagram from '#lib/components/MouseDiagram.svelte';
	import GenericMouse from '#lib/components/GenericMouse.svelte';
	import CheckNotice from '#lib/components/CheckNotice.svelte';
	import { daemon, getDesk } from '#lib/api.ts';
	import { pipe, loadDevices, errorText, experimentalBadge } from '#lib/daemon.svelte.ts';
	import { locked } from '#lib/checks.ts';
	import { GEL_NAMES, KEY_GROUPS, actionName, MODIFIERS, MOUSE_BUTTONS, describeFunction, gelFor, keyTitle, parseSpec, shortLabel, toSpec, type Gel, type Mapping } from '#lib/keys.ts';
	import { ms } from '#lib/motion.ts';
	import type { Capabilities, Config, DeskDevice, FeatureCheck, KeyMapping, Layer, WriteResult } from '#lib/types.ts';
	import type { Component } from 'svelte';

	let { config, deviceId }: { config: Config; deviceId: string } = $props();

	let desk = $state<DeskDevice[]>([]);
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

	const device = $derived(pipe.devices.find((d) => d.id === deviceId) ?? null);
	const isMouse = $derived(device?.kind === 'mouse');
	const word = $derived(isMouse ? 'mouse' : 'keyboard');

	async function load(id: string, l: Layer) {
		loading = true;
		loadError = null;
		try {
			// Capabilities first: some devices (most mice without a Hypershift button) only have the normal layer.
			const c = caps?.id === id ? caps : await daemon<Capabilities>('capabilities', id);
			if (!c.keymap_layers.includes(l)) {
				caps = c;
				layer = c.keymap_layers[0] ?? 'normal';
				return;
			}
			const [r, n] = await Promise.all([
				daemon<KeyMapping[]>('keymap.dump', id, { layer: l }),
				l === 'normal' ? Promise.resolve(null) : daemon<KeyMapping[]>('keymap.dump', id, { layer: 'normal' })
			]);
			if (l !== layer) return;
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
		if (device) load(deviceId, layer);
	});

	const selected = $derived(rows.find((k) => k.key === selectedId) ?? null);
	const normalById = $derived(new Map(normal.map((k) => [k.key, k.function])));
	/** Does this key do something other than its normal job on the layer shown? */
	const changed = (k: KeyMapping) => (layer === 'normal' ? /^(razer|media|macro|off|power|profile|dpi|lighting|shortcut)\b/.test(k.function) : normalById.get(k.key) !== k.function);
	const gelOf = (k: KeyMapping): Gel | null => gelFor(k.function, changed(k), deviceId);
	const action = (k: { function: string; description: string }) => actionName(k.function, k.description, deviceId);
	const layers = $derived(caps?.keymap_layers ?? ['normal', 'hypershift']);

	// Experimental devices: remapping stays off until the read-only keymap check passes.
	const isLocked = $derived(locked(caps, 'keymap'));
	const onchecks = (checks: FeatureCheck[]) => caps && (caps = { ...caps, checks });

	/** Mice with their own drawing; every other mouse gets the plain one. */
	type Diagram = Component<{ regions: Map<string, { name: string; label: string; gel: Gel | null; selected: boolean }>; onselect: (name: string) => void }>;
	const DIAGRAMS: Record<string, Diagram> = { 'razer-basilisk-v3-pro': MouseDiagram };
	const Mouse = $derived(DIAGRAMS[deviceId] ?? GenericMouse);
	const changes = $derived(rows.filter((k) => gelOf(k)));

	// Keycaps for the drawing, keyed by the layout's shape names (the keymap's LED names).
	const board = $derived(desk.find((d) => d.id === deviceId && d.kind === 'keyboard') ?? null);
	const keyByLed = $derived(new Map((caps?.keys ?? []).filter((k) => k.led).map((k) => [k.led!, k.id])));
	const ledById = $derived(new Map((caps?.keys ?? []).filter((k) => k.led).map((k) => [k.id, k.led!])));
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
			const gel = gelOf(k);
			m.set(s.name, { sub: gel ? shortLabel(k.function, deviceId) : undefined, gel: gel ?? undefined, label: `${legendFor(s.name) || 'Space'}: ${k.description}` });
		}
		return m;
	});
	// Mouse: every button as a region on the drawing.
	const mouseRegions = $derived(new Map(rows.map((k) => [k.name, { name: k.name, label: `${keyTitle(k.name)}: ${action(k)}`, gel: gelOf(k), selected: k.key === selectedId }])));
	const unit = $derived(isMouse ? ['button', 'buttons'] : ['key', 'keys']);

	const keyName = (k: KeyMapping) => {
		const led = ledById.get(k.key);
		return led ? legendFor(led) || 'Space' : keyTitle(k.name);
	};

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
		if (!selected) return;
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
					? { ok: true, text: `Saved. The ${word} reads back: ${action(r.after)}.` }
					: { ok: false, text: `Saved, but the ${word} reads back: ${action(r.after)}.` };
		} catch (e) {
			outcome = { ok: false, text: errorText(e) };
		} finally {
			busy = false;
		}
	}

	const layerOptions = $derived([
		{ value: 'normal' as const, label: 'Normal' },
		{ value: 'hypershift' as const, label: isMouse ? 'With Hypershift held' : 'With Fn held' }
	]);
</script>

{#if !pipe.loaded}
	<p class="loading">Connecting to the engine…</p>
{:else if pipe.unreachable || !device}
	<PipeUnavailable page={isMouse ? 'buttons' : 'keys'} unreachable={pipe.unreachable} />
{:else}
	<Workspace title={pageTitle(isMouse ? 'buttons' : 'keys')} badge={experimentalBadge(deviceId)} subtitle="Changes are saved in the {word} itself, so they keep working without uncoil." panelLabel={isMouse ? 'Selected button' : 'Selected key'}>
		{#snippet tools()}
			{#if layers.length > 1}
				<div class="layer"><Segmented label="Layer" options={layerOptions} value={layer} onchange={(v) => (layer = v)} /></div>
			{/if}
		{/snippet}

		<div class="map" aria-busy={loading}>
			{#if loadError}
				<p class="outcome bad" role="alert">{loadError}</p>
			{:else if board && caps}
				<Keyboard device={board} caps={capMap} selected={selectedId !== null ? (ledById.get(selectedId) ?? null) : null} onselect={(led) => keyByLed.has(led) && (selectedId = keyByLed.get(led)!)} />
			{:else}
				<div class="mouse-stage">
				<Mouse regions={mouseRegions} onselect={(name) => (selectedId = rows.find((k) => k.name === name)?.key ?? selectedId)} />
				<div class="buttons" role="listbox" aria-label="Mouse buttons">
					{#each rows as k (k.key)}
						{@const gel = gelOf(k)}
						<button type="button" role="option" aria-selected={k.key === selectedId} onclick={() => (selectedId = k.key)}>
							<span class="bname">{keyTitle(k.name)}</span>
							<span class="bdesc">{action(k)}</span>
							{#if gel}<span class="gel" style:background="var(--color-gel-{gel})" title={GEL_NAMES[gel]}></span>{/if}
						</button>
					{/each}
				</div>
				</div>
			{/if}
		</div>

		<section class="changes" aria-labelledby="changes-title">
			<h2 id="changes-title" class="section-title">
				{layer === 'hypershift' ? (isMouse ? 'What Hypershift changes' : 'What Fn changes') : isMouse ? 'Buttons doing something special' : 'Keys doing something special'}
				<span class="count">{changes.length} {changes.length === 1 ? unit[0] : unit[1]}{board ? ' · Win and Fn can’t be changed' : ''}</span>
			</h2>
			{#if changes.length}
				<div class="rows">
					{#each changes as k (k.key)}
						{@const gel = gelOf(k)!}
						<button type="button" class="row" aria-pressed={k.key === selectedId} onclick={() => (selectedId = k.key)}>
							<span class="mk">{keyName(k)}</span>
							<span class="what">{action(k)}</span>
							<span class="kind"><span class="gel" style:background="var(--color-gel-{gel})"></span>{GEL_NAMES[gel]}</span>
						</button>
					{/each}
				</div>
			{:else}
				<p class="empty">{isMouse ? 'Every button does its usual job on this layer.' : 'Every key sends its own key on this layer.'}</p>
			{/if}
		</section>

		{#snippet panel()}
			{#if selected}
				{#key `${selected.key}-${layer}`}
					<div class="insp" in:fade={{ duration: ms(140) }}>
						<h2 class="kname">{layer === 'hypershift' ? (isMouse ? 'Hypershift + ' : 'Fn + ') : ''}{keyName(selected)}</h2>
						<p class="now"><span class="label">Does now</span>{action(selected)}</p>

						<CheckNotice {deviceId} {caps} features={['keymap']} {onchecks} compact />

						<fieldset class="edit" disabled={isLocked}>
						<div class="search">
							<Search size={14} />
							<input class="input" type="search" placeholder="Search keys and actions" aria-label="Search keys and actions" autocomplete="off" bind:value={query} />
						</div>
						<div class="options" role="listbox" aria-label="Change to">
							{#each filtered as [group, opts] (group)}
								<p class="group">{group}</p>
								{#each opts as o (o.spec)}
									<button type="button" role="option" aria-selected={o.spec === draftBase} onclick={() => choose(o.spec)}>
										{o.label}{#if o.spec === selected.function}<span class="cur">current</span>{/if}
									</button>
								{/each}
							{:else}
								<p class="empty">Nothing matches “{query}”.</p>
							{/each}
						</div>

						{#if draft.type === 'key'}
							<div class="mods-wrap">
								<p class="label">Hold together with</p>
								<div class="mods" role="group" aria-label="Hold together with">
									{#each MODIFIERS as m (m.spec)}
										<button type="button" class="mod" aria-pressed={draft.mods.includes(m.spec)} onclick={() => toggleMod(m.spec)}>{m.label}</button>
									{/each}
								</div>
							</div>
						{:else if draft.type === 'other'}
							<p class="note">This key uses a kind of action uncoil can't edit yet (macro, DPI, profile or a Synapse-only key). Pick something above to replace it.</p>
						{/if}
						</fieldset>

						{#if dirty}
							<p class="now" in:fade={{ duration: ms(140) }}><span class="label">Will do</span>{actionName(draftSpec, describeFunction(draftSpec, deviceId), deviceId)}</p>
						{/if}

						<div class="actions">
							<WriteButton
								label="Save to {word}"
								warning="This is saved in the {word}'s own memory, so it works even without uncoil. Restore original puts back what was there before."
								disabled={!dirty || isLocked}
								{busy}
								onconfirm={() => write('keymap.set')}
							/>
							<WriteButton quiet label="Restore original" warning="Puts back what this {unit[0]} did before uncoil changed it (or the factory setting)." disabled={isLocked} {busy} onconfirm={() => write('keymap.reset')} />
						</div>
						{#if outcome}
							<p class="outcome" class:bad={!outcome.ok} role="status" in:fade={{ duration: ms(160) }}>{outcome.text}</p>
						{/if}
					</div>
				{/key}
			{:else}
				<p class="note">Pick a {unit[0]} to change it.</p>
			{/if}
		{/snippet}
	</Workspace>
{/if}

<style>
	.loading {
		margin: 24px 28px;
		color: var(--color-ink-3);
	}
	.layer {
		width: 250px;
	}
	.map {
		container: map / inline-size;
		display: grid;
		justify-items: center;
		transition: opacity var(--t-mid) var(--ease);
	}
	.map[aria-busy='true'] {
		opacity: 0.6;
	}
	.map :global(.board) {
		max-width: 900px;
	}
	.mouse-stage {
		display: grid;
		grid-template-columns: minmax(280px, 420px) minmax(0, 1fr);
		gap: 28px;
		align-items: center;
		width: 100%;
	}
	.buttons {
		display: grid;
		gap: 6px;
		width: 100%;
	}
	.buttons button {
		display: grid;
		grid-template-columns: 140px 1fr auto;
		align-items: center;
		gap: 12px;
		height: 40px;
		padding: 0 12px;
		border: var(--hair);
		border-radius: var(--radius);
		background: var(--color-surface);
		text-align: left;
	}
	.buttons button:hover {
		border-color: var(--color-seam-2);
	}
	.buttons button[aria-selected='true'] {
		border-color: var(--color-select);
		box-shadow: inset 0 0 0 1px var(--color-select);
	}
	.bname {
		font-weight: 600;
	}
	.bdesc {
		color: var(--color-ink-3);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.changes {
		display: grid;
		gap: 10px;
		container-type: inline-size;
	}
	@container (max-width: 600px) {
		.changes .rows {
			grid-template-columns: minmax(0, 1fr);
		}
	}
	.count {
		margin-left: 6px;
		color: var(--color-ink-3);
		font-family: var(--font-sans);
		font-size: 12px;
		font-weight: 400;
	}
	.rows {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 6px 16px;
	}
	.row {
		display: grid;
		grid-template-columns: 52px minmax(0, 1fr) auto;
		align-items: center;
		gap: 12px;
		height: 38px;
		padding: 0 10px 0 6px;
		border: 1px solid transparent;
		border-radius: var(--radius);
		background: var(--color-surface);
		text-align: left;
	}
	.row:hover {
		border-color: var(--color-seam-2);
	}
	.row[aria-pressed='true'] {
		border-color: var(--color-select);
	}
	.mk {
		display: grid;
		place-items: center;
		height: 26px;
		border-radius: var(--radius-sm);
		background: var(--cap);
		box-shadow: inset 0 -2px 0 var(--cap-edge), 0 0 0 1px var(--cap-edge);
		font-size: 11.5px;
		font-weight: 600;
	}
	.what {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.kind {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--color-ink-3);
		font-size: 12px;
		white-space: nowrap;
	}
	.empty,
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
	}
	.insp,
	.edit {
		display: grid;
		gap: 14px;
	}
	.edit {
		min-width: 0;
		margin: 0;
		padding: 0;
		border: 0;
		transition: opacity var(--t-mid) var(--ease);
	}
	.edit:disabled {
		opacity: 0.45;
	}
	.kname {
		margin: 0;
		font-family: var(--font-display);
		font-size: 22px;
		font-weight: 600;
	}
	.now {
		display: grid;
		gap: 2px;
		margin: 0;
		font-size: 14px;
		font-weight: 600;
	}
	.now .label {
		font-weight: 500;
	}
	.search {
		position: relative;
		display: grid;
		align-items: center;
		color: var(--color-ink-3);
	}
	.search :global(svg) {
		position: absolute;
		left: 10px;
		pointer-events: none;
	}
	.search input {
		width: 100%;
		padding-left: 30px;
	}
	.options {
		display: grid;
		max-height: 240px;
		overflow: auto;
		padding: 4px;
		border: var(--hair);
		border-radius: var(--radius);
		background: var(--color-surface);
	}
	.group {
		margin: 8px 8px 3px;
		color: var(--color-ink-3);
		font-size: 11.5px;
		font-weight: 500;
	}
	.group:first-child {
		margin-top: 4px;
	}
	.options button {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 30px;
		padding: 0 8px;
		border: 0;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--color-ink-2);
		text-align: left;
	}
	.options button:hover {
		background: var(--color-surface-2);
		color: var(--color-ink);
	}
	.options button[aria-selected='true'] {
		background: var(--color-surface-2);
		color: var(--color-ink);
		font-weight: 600;
	}
	.cur {
		color: var(--color-ink-3);
		font-size: 11px;
		font-weight: 400;
	}
	.mods-wrap {
		display: grid;
		gap: 6px;
	}
	.mods-wrap .label {
		margin: 0;
	}
	.mods {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 6px;
	}
	.mod {
		height: 30px;
		border: var(--hair-strong);
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-2);
		font-weight: 500;
		transition:
			background-color var(--t-mid) var(--ease),
			color var(--t-mid) var(--ease);
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
	@container map (max-width: 600px) {
		.mouse-stage {
			grid-template-columns: minmax(0, 1fr);
			justify-items: center;
		}
		.buttons {
			max-width: 520px;
		}
	}
</style>
