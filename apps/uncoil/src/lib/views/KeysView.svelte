<script lang="ts">
	import { onMount } from 'svelte';
	import { fade, fly } from 'svelte/transition';
	import { expoOut } from 'svelte/easing';
	import Segmented from '#lib/components/Segmented.svelte';
	import WriteButton from '#lib/components/WriteButton.svelte';
	import { daemon, getDesk } from '#lib/api.ts';
	import { pipe, loadDevices, withFeature, shortName, errorText } from '#lib/daemon.svelte.ts';
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
		if (!deviceId && devices.length) deviceId = (devices.find((d) => d.kind === 'keyboard') ?? devices[0]).id;
	});
	const device = $derived(devices.find((d) => d.id === deviceId) ?? null);

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
	const byLed = $derived(new Map((caps?.keys ?? []).filter((k) => k.led).map((k) => [k.led!, k.id])));
	const rowById = $derived(new Map(rows.map((k) => [k.key, k])));
	const normalById = $derived(new Map(normal.map((k) => [k.key, k.function])));
	const special = (k: KeyMapping) => layer !== 'normal' && normalById.get(k.key) !== k.function;

	// Keyboard drawing, to scale, from the desk layout.
	const board = $derived(desk.find((d) => d.id === deviceId && d.kind === 'keyboard') ?? null);

	// ---- editor -------------------------------------------------------------------------------------
	let draft = $state<Mapping>({ type: 'off' });
	$effect(() => {
		if (selected) draft = parseSpec(selected.function);
	});
	const draftSpec = $derived(toSpec(draft));
	const dirty = $derived(!!selected && draftSpec !== selected.function && !(draft.type === 'key' && draft.key === 'NONE' && !draft.mods.length && selected.function === 'key NONE'));
	const isMouse = $derived(device?.kind === 'mouse');
	const typeOptions = $derived(
		isMouse
			? [
					{ value: 'button' as const, label: 'Mouse' },
					{ value: 'key' as const, label: 'Key' },
					{ value: 'off' as const, label: 'Off' }
				]
			: [
					{ value: 'key' as const, label: 'Key' },
					{ value: 'button' as const, label: 'Mouse' },
					{ value: 'off' as const, label: 'Off' }
				]
	);
	function setType(t: 'key' | 'button' | 'off') {
		draft = t === 'key' ? { type: 'key', key: 'PRINT_SCREEN', mods: [] } : t === 'button' ? { type: 'button', button: 1 } : { type: 'off' };
	}
	function toggleMod(spec: string) {
		if (draft.type !== 'key') return;
		draft = { ...draft, mods: draft.mods.includes(spec) ? draft.mods.filter((m) => m !== spec) : [...draft.mods, spec] };
	}

	let busy = $state(false);
	let outcome = $state<{ ok: boolean; text: string } | null>(null);
	$effect(() => {
		void selectedId;
		void layer;
		outcome = null;
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
				? { ok: true, text: 'Already set that way; nothing was written.' }
				: r.verified
					? { ok: true, text: `Written and read back: ${r.after.description}.` }
					: { ok: false, text: `Written, but the device reads back ${r.after.description}.` };
		} catch (e) {
			outcome = { ok: false, text: errorText(e) };
		} finally {
			busy = false;
		}
	}

	const deviceWord = $derived(device?.kind === 'mouse' ? 'mouse' : 'keyboard');
	const layerOptions = [
		{ value: 'normal' as const, label: 'Normal', code: 'L0' },
		{ value: 'hypershift' as const, label: 'Fn layer', code: 'L1' }
	];
</script>

<section class="keys" aria-labelledby="keys-title">
	<header class="head">
		<h1 id="keys-title" class="title"><span class="display fac">FAC 200</span><span class="caps name">Keys</span></h1>
		<p class="lede">Mappings are stored in the device and keep working without uncoil.</p>
	</header>

	{#if !pipe.loaded}
		<p class="state">Asking uncoild…</p>
	{:else if pipe.unreachable}
		<div class="state" in:fade={{ duration: ms(260) }}>
			<p class="caps">uncoild isn't answering on its control pipe</p>
			<p>Key mapping needs uncoild 0.2 or later running. Install it with <code>scripts\install-task.ps1</code>, then try again.</p>
			<button type="button" class="ghost caps" onclick={loadDevices}>Try again</button>
		</div>
	{:else if !devices.length}
		<p class="state">No connected device supports key mapping.</p>
	{:else}
		<div class="bar">
			<Segmented
				label="Device"
				options={devices.map((d, i) => ({ value: d.id, label: d.kind === 'keyboard' ? 'Keyboard' : d.kind === 'mouse' ? 'Mouse' : shortName(d.name), code: `20${i + 1}` }))}
				value={deviceId ?? ''}
				onchange={(v) => (deviceId = v)}
			/>
			<Segmented label="Layer" options={layerOptions} value={layer} onchange={(v) => (layer = v)} />
		</div>

		<div class="body">
			<div class="map" aria-busy={loading}>
				{#if loadError}
					<p class="err" role="alert">{loadError}</p>
				{:else if board && caps}
					<svg viewBox="{board.x - 0.25} {board.y - 0.25} {board.w + 0.5} {board.h + 0.5}" role="group" aria-label="Keyboard keys">
						<rect x={board.x} y={board.y} width={board.w} height={board.h} class="frame" />
						{#each board.shapes.filter((s) => s.is_key) as s (s.name)}
							{@const id = byLed.get(s.name)}
							{@const k = id !== undefined ? rowById.get(id) : undefined}
							{#if k}
								<g
									class="key mappable"
									class:special={special(k)}
									class:sel={k.key === selectedId}
									role="button"
									tabindex="0"
									aria-label="{keyTitle(k.name)}: {k.description}"
									aria-pressed={k.key === selectedId}
									onclick={() => (selectedId = k.key)}
									onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), (selectedId = k.key))}
								>
									<rect x={s.x - s.w / 2 + 0.06} y={s.y - s.h / 2 + 0.06} width={s.w - 0.12} height={s.h - 0.12} />
									<text x={s.x} y={s.y} dominant-baseline="central" text-anchor="middle">{shortLabel(k.function)}</text>
								</g>
							{:else}
								<g class="key" aria-label="{s.name} (not remappable)" role="img">
									<rect x={s.x - s.w / 2 + 0.06} y={s.y - s.h / 2 + 0.06} width={s.w - 0.12} height={s.h - 0.12} />
									<text x={s.x} y={s.y} dominant-baseline="central" text-anchor="middle">{s.name.split(' ').pop()}</text>
								</g>
							{/if}
						{/each}
					</svg>
					<p class="legend caps-sm">
						{#if layer === 'hypershift'}<span class="sw special"></span>Differs with Fn{:else}<span class="sw"></span>Shows what each key sends{/if}
						<span class="sw off"></span>Fixed
					</p>
				{:else}
					<div class="buttons" role="listbox" aria-label="Mouse buttons">
						{#each rows as k (k.key)}
							<button type="button" role="option" aria-selected={k.key === selectedId} class:special={special(k)} onclick={() => (selectedId = k.key)}>
								<span class="code num">{String(k.key).padStart(3, '0')}</span>
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
						<div class="insp" in:fly={{ x: 10, duration: ms(320), easing: expoOut }}>
							<h2 class="kname">{keyTitle(selected.name)}<span class="caps-sm layer">{layer === 'hypershift' ? 'With Fn' : 'Normal'}</span></h2>
							<dl>
								<div><dt class="caps-sm">Sends now</dt><dd>{selected.description}</dd></div>
								<div><dt class="caps-sm">Code</dt><dd class="spec">{selected.function}</dd></div>
							</dl>

							{#if draft.type === 'other'}
								<p class="note">This mapping type (macro, DPI, profile, Razer key) can't be edited here yet. Pick a new type to replace it.</p>
							{/if}
							<Segmented label="Map to" options={typeOptions} value={draft.type === 'other' ? (null as never) : draft.type} onchange={setType} />

							{#if draft.type === 'key'}
								<label class="field">
									<span class="caps-sm">Key</span>
									<select value={draft.key} onchange={(e) => draft.type === 'key' && (draft = { ...draft, key: e.currentTarget.value })}>
										<option value="NONE">None (modifiers only)</option>
										{#each KEY_GROUPS as g (g.label)}
											<optgroup label={g.label}>
												{#each g.keys as key (key)}<option value={key}>{keyTitle(key)}</option>{/each}
											</optgroup>
										{/each}
									</select>
								</label>
								<div class="mods" role="group" aria-label="Modifiers">
									{#each MODIFIERS as m (m.spec)}
										<button type="button" class="mod caps-sm" aria-pressed={draft.mods.includes(m.spec)} onclick={() => toggleMod(m.spec)}>{m.label}</button>
									{/each}
								</div>
							{:else if draft.type === 'button'}
								<label class="field">
									<span class="caps-sm">Button</span>
									<select value={draft.button} onchange={(e) => (draft = { type: 'button', button: Number(e.currentTarget.value) })}>
										{#each MOUSE_BUTTONS as b (b.value)}<option value={b.value}>{b.label}</option>{/each}
									</select>
								</label>
							{/if}

							<p class="preview"><span class="caps-sm">Will send</span>{describeFunction(draftSpec)}</p>

							<div class="actions">
								<WriteButton
									label="Write to {deviceWord}"
									warning="This is saved in the {deviceWord}'s own memory, so it works without uncoil. Restore original puts back what was there before uncoil changed it."
									disabled={!dirty || draft.type === 'other'}
									{busy}
									onconfirm={() => write('keymap.set')}
								/>
								<WriteButton
									label="Restore original"
									warning="Puts back the mapping this key had before uncoil first changed it (or the factory mapping)."
									{busy}
									onconfirm={() => write('keymap.reset')}
								/>
							</div>
							{#if outcome}
								<p class="outcome" class:bad={!outcome.ok} role="status" in:fade={{ duration: ms(200) }}>{outcome.text}</p>
							{/if}
						</div>
					{/key}
				{:else}
					<p class="note">Pick a key to see what it sends.</p>
				{/if}
			</aside>
		</div>
	{/if}
</section>

<style>
	.keys {
		display: grid;
		grid-template-rows: auto auto 1fr;
		height: 100%;
		min-height: 0;
		border: var(--hair);
	}
	.head {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: 24px;
		padding: 22px 24px 18px;
		border-bottom: var(--hair);
	}
	.title {
		display: flex;
		align-items: baseline;
		gap: 18px;
		margin: 0;
		font-weight: inherit;
	}
	.fac {
		font-size: 44px;
		white-space: nowrap;
	}
	.name {
		color: var(--color-ink-2);
		letter-spacing: 0.32em;
	}
	.lede {
		margin: 0;
		color: var(--color-ink-2);
		font-size: 13px;
		text-align: right;
	}
	.state {
		display: grid;
		gap: 12px;
		align-content: start;
		justify-items: start;
		padding: 24px;
		margin: 0;
		color: var(--color-ink-3);
		font-size: 13px;
	}
	.state p {
		margin: 0;
		max-width: 60ch;
		color: var(--color-ink-2);
	}
	.state .caps {
		color: var(--color-ink);
	}
	code,
	.spec {
		font-family: ui-monospace, 'Cascadia Mono', Consolas, monospace;
		font-size: 12px;
		color: var(--color-ink);
	}
	.bar {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 0.8fr);
		gap: 18px;
		padding: 16px 24px;
		border-bottom: var(--hair);
	}
	.body {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 320px;
		min-height: 0;
	}
	.map {
		display: grid;
		align-content: center;
		gap: 12px;
		padding: 20px 24px;
		border-right: var(--hair);
		min-height: 0;
		overflow: auto;
		transition: opacity var(--t-mid) var(--ease);
	}
	.map[aria-busy='true'] {
		opacity: 0.5;
	}
	svg {
		width: 100%;
		height: auto;
		max-height: 100%;
	}
	.frame {
		fill: none;
		stroke: var(--color-seam-2);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
	}
	.key rect {
		fill: var(--color-ground);
		stroke: var(--color-seam-2);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
		transition:
			stroke var(--t-mid) var(--ease),
			fill var(--t-mid) var(--ease);
	}
	.key text {
		font-size: 0.24px;
		font-stretch: 108%;
		letter-spacing: 0.02em;
		fill: var(--color-ink-4);
		pointer-events: none;
	}
	.key.mappable text {
		fill: var(--color-ink-3);
	}
	.key.mappable {
		cursor: default;
	}
	.key.mappable:hover rect {
		stroke: var(--color-ink-3);
	}
	.key.special text {
		fill: var(--color-ink);
	}
	.key.special rect {
		fill: var(--color-raised);
		stroke: var(--color-ink-3);
	}
	.key.sel rect {
		stroke: var(--color-ink);
	}
	.key.sel text {
		fill: var(--color-fac-red);
	}
	.key:focus-visible {
		outline: none;
	}
	.key:focus-visible rect {
		stroke: var(--color-ink);
		stroke-dasharray: 3 2;
	}
	.legend {
		display: flex;
		white-space: nowrap;
		align-items: center;
		gap: 8px;
		margin: 0;
		color: var(--color-ink-4);
	}
	.sw {
		width: 10px;
		height: 10px;
		border: 1px solid var(--color-seam-2);
	}
	.sw.special {
		border-color: var(--color-ink-3);
		background: var(--color-raised);
	}
	.sw.off {
		margin-left: 14px;
	}
	.buttons {
		display: grid;
		align-self: start;
	}
	.buttons button {
		display: grid;
		grid-template-columns: 44px 160px 1fr;
		align-items: center;
		gap: 10px;
		height: 38px;
		padding: 0 12px;
		border: 1px solid transparent;
		border-bottom-color: var(--color-seam);
		background: none;
		color: var(--color-ink-2);
		text-align: left;
		font-size: 13px;
		transition:
			color var(--t-mid) var(--ease),
			border-color var(--t-mid) var(--ease);
	}
	.buttons button:hover {
		color: var(--color-ink);
	}
	.buttons button[aria-selected='true'] {
		border-color: var(--color-ink);
		color: var(--color-ink);
	}
	.buttons button[aria-selected='true'] .code {
		color: var(--color-fac-red);
	}
	.buttons .code {
		font-size: 12px;
		color: var(--color-ink-4);
		letter-spacing: 0.06em;
	}
	.bdesc {
		color: var(--color-ink-3);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.inspector {
		padding: 20px 22px;
		min-height: 0;
		min-width: 0;
		overflow: hidden auto;
	}
	.insp {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 16px;
	}
	.kname {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		margin: 0;
		font-size: 22px;
		font-weight: 400;
		font-stretch: 118%;
	}
	.layer {
		color: var(--color-ink-3);
	}
	dl {
		display: grid;
		gap: 10px;
		margin: 0;
		padding-bottom: 14px;
		border-bottom: var(--hair);
	}
	dl div {
		display: grid;
		gap: 3px;
	}
	dt {
		color: var(--color-ink-3);
	}
	dd {
		margin: 0;
		font-size: 13px;
	}
	.field {
		display: grid;
		gap: 8px;
	}
	.field span {
		color: var(--color-ink-3);
	}
	select {
		appearance: none;
		height: 36px;
		padding: 0 12px;
		border: var(--hair-strong);
		border-radius: 0;
		background: var(--color-ground)
			url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6'%3E%3Cpath d='M0 0h10L5 6z' fill='%23a8a8a8'/%3E%3C/svg%3E")
			no-repeat right 12px center;
		color: var(--color-ink);
		font: inherit;
		font-size: 13px;
		transition: border-color var(--t-mid) var(--ease);
	}
	select:hover {
		border-color: var(--color-ink-3);
	}
	optgroup,
	option {
		background: var(--color-raised);
		color: var(--color-ink);
	}
	.mods {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 6px;
	}
	.mod {
		height: 30px;
		border: var(--hair-strong);
		background: none;
		color: var(--color-ink-3);
		transition:
			color var(--t-mid) var(--ease),
			border-color var(--t-mid) var(--ease);
	}
	.mod:hover {
		color: var(--color-ink-2);
	}
	.mod[aria-pressed='true'] {
		border-color: var(--color-ink);
		color: var(--color-ink);
	}
	.preview {
		display: grid;
		gap: 3px;
		margin: 0;
		font-size: 13px;
	}
	.preview .caps-sm {
		color: var(--color-ink-3);
	}
	.actions {
		display: grid;
		gap: 10px;
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
	}
	.outcome {
		display: grid;
		grid-template-columns: 7px 1fr;
		gap: 10px;
		align-items: baseline;
		margin: 0;
		font-size: 12px;
		line-height: 1.5;
		color: var(--color-ink-2);
	}
	.outcome::before {
		content: '';
		width: 7px;
		height: 7px;
		background: var(--color-ink-3);
	}
	.outcome.bad {
		color: var(--color-fac-yellow);
	}
	.outcome.bad::before {
		background: var(--color-fac-yellow);
	}
	.err {
		color: var(--color-fac-yellow);
		font-size: 13px;
	}
	.ghost {
		height: 34px;
		padding: 0 14px;
		border: var(--hair-strong);
		background: none;
		color: var(--color-ink-2);
	}
	.ghost:hover {
		color: var(--color-ink);
		border-color: var(--color-ink-3);
	}
	@container view (max-width: 820px) {
		.body {
			grid-template-columns: minmax(0, 1fr);
		}
		.map {
			border-right: 0;
			border-bottom: var(--hair);
		}
		.keys {
			height: auto;
			min-height: 100%;
		}
	}
</style>
