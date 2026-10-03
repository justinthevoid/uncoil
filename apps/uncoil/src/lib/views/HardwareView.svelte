<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import Segmented from '#lib/components/Segmented.svelte';
	import CatalogList from '#lib/components/CatalogList.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import WriteButton from '#lib/components/WriteButton.svelte';
	import { daemon } from '#lib/api.ts';
	import { pipe, loadDevices, withFeature, shortName, errorText } from '#lib/daemon.svelte.ts';
	import { DIAL_MODES } from '#lib/keys.ts';
	import { ms } from '#lib/motion.ts';
	import type { Capabilities, DialState, EffectState, OledState, ProfileInfo, WriteResult } from '#lib/types.ts';

	type Note = { ok: boolean; text: string } | null;

	onMount(() => {
		loadDevices();
	});

	const kindLabel = (k: string) => (k === 'keyboard' ? 'Keyboard' : k === 'mouse' ? 'Mouse' : k === 'mousemat' ? 'Mat' : 'Device');

	// ---- 310 firmware effects -------------------------------------------------------------------------
	const fxDevices = $derived(withFeature('hw_effects'));
	let fxId = $state<string | null>(null);
	$effect(() => {
		if (!fxId && fxDevices.length) fxId = fxDevices[0].id;
	});
	const fxDevice = $derived(fxDevices.find((d) => d.id === fxId) ?? null);
	let fxCaps = $state<Capabilities | null>(null);
	$effect(() => {
		const id = fxId;
		if (id) daemon<Capabilities>('capabilities', id).then((c) => id === fxId && (fxCaps = c)).catch(() => (fxCaps = null));
	});
	let fxName = $state('spectrum');
	let fxColor = $state('#e2372c');
	let fxColor2 = $state('#2f63e8');
	let fxTwo = $state(false);
	let fxDir = $state<'left' | 'right'>('left');
	let fxSpeed = $state(40);
	let fxBusy = $state(false);
	let fxNote = $state<Note>(null);
	const FX_LABEL: Record<string, string> = {
		off: 'Off',
		static: 'Static',
		breathing: 'Breathing',
		spectrum: 'Spectrum',
		wave: 'Wave',
		wheel: 'Wheel',
		reactive: 'Reactive',
		starlight: 'Starlight'
	};
	const fxSpec = $derived.by(() => {
		switch (fxName) {
			case 'static':
			case 'reactive':
				return `${fxName} ${fxColor}`;
			case 'breathing':
			case 'starlight':
				return fxTwo ? `${fxName} ${fxColor} ${fxColor2}` : `${fxName} ${fxColor}`;
			case 'wave':
			case 'wheel':
				return `${fxName} ${fxDir} speed ${fxSpeed}`;
			default:
				return fxName;
		}
	});
	async function runEffect(software: boolean) {
		if (!fxId) return;
		fxBusy = true;
		fxNote = null;
		try {
			const r = software
				? await daemon<EffectState>('effect.software', fxId)
				: await daemon<EffectState>('effect.hw', fxId, { effect: fxSpec });
			pipe.devices = pipe.devices.map((d) => (d.id === fxId ? { ...d, hw_effect: r.effect } : d));
			const fx = r.effect ? (FX_LABEL[r.effect.split(' ')[0]] ?? r.effect).toLowerCase() : null;
			fxNote = { ok: true, text: fx ? `The ${kindLabel(fxDevice!.kind).toLowerCase()} is running its own ${fx} effect.` : 'Back on uncoil’s desk-wide effect.' };
		} catch (e) {
			fxNote = { ok: false, text: errorText(e) };
		} finally {
			fxBusy = false;
		}
	}

	// ---- 320 command dial / 330 OLED ----------------------------------------------------------------
	const dialDevice = $derived(withFeature('dial')[0] ?? null);
	const oledDevice = $derived(withFeature('oled')[0] ?? null);
	let dial = $state<DialState | null>(null);
	let dialModes = $state<string[]>([]);
	let dialChoice = $state<string | null>(null);
	let dialBusy = $state(false);
	let dialNote = $state<Note>(null);
	$effect(() => {
		const d = dialDevice;
		if (!d) return;
		Promise.all([daemon<DialState>('dial.get', d.id), daemon<Capabilities>('capabilities', d.id)])
			.then(([s, c]) => {
				dial = s;
				dialModes = c.dial_modes;
				dialChoice = s.mode;
			})
			.catch((e) => (dialNote = { ok: false, text: errorText(e) }));
	});
	async function saveDial() {
		if (!dialDevice || !dialChoice) return;
		dialBusy = true;
		dialNote = null;
		try {
			const r = await daemon<WriteResult<DialState>>('dial.set', dialDevice.id, { mode: dialChoice, write: true });
			dial = r.after;
			dialNote = r.unchanged
				? { ok: true, text: 'Already in that mode; nothing was written.' }
				: { ok: r.verified, text: r.verified ? `Saved: the dial is now ${DIAL_MODES[r.after.mode ?? ''] ?? r.after.mode}.` : 'Written, but the keyboard reads back a different mode.' };
		} catch (e) {
			dialNote = { ok: false, text: errorText(e) };
		} finally {
			dialBusy = false;
		}
	}

	let oled = $state<OledState | null>(null);
	let oledBright = $state(100);
	let oledBusy = $state(false);
	let oledNote = $state<Note>(null);
	$effect(() => {
		const d = oledDevice;
		if (!d) return;
		daemon<OledState>('oled.get', d.id)
			.then((s) => {
				oled = s;
				oledBright = s.brightness ?? 100;
			})
			.catch((e) => (oledNote = { ok: false, text: errorText(e) }));
	});
	async function saveOled() {
		if (!oledDevice) return;
		oledBusy = true;
		oledNote = null;
		try {
			const r = await daemon<WriteResult<number>>('oled.set', oledDevice.id, { brightness: Math.round(oledBright), write: true });
			if (oled) oled.brightness = r.after;
			oledNote = r.unchanged ? { ok: true, text: 'Already at that brightness.' } : { ok: r.verified, text: `Saved: screen at ${r.after}%.` };
		} catch (e) {
			oledNote = { ok: false, text: errorText(e) };
		} finally {
			oledBusy = false;
		}
	}

	// ---- 340 profiles -------------------------------------------------------------------------------
	let profiles = $state<{ name: string; kind: string; info: ProfileInfo | null; error: string | null }[]>([]);
	$effect(() => {
		const list = withFeature('profiles');
		Promise.all(
			list.map((d) =>
				daemon<ProfileInfo>('profile.list', d.id)
					.then((info) => ({ name: shortName(d.name), kind: d.kind, info, error: null }))
					.catch((e) => ({ name: shortName(d.name), kind: d.kind, info: null, error: errorText(e) }))
			)
		).then((p) => (profiles = p));
	});
</script>

<section class="hw" aria-labelledby="hw-title">
	<header class="head">
		<h1 id="hw-title" class="title"><span class="display fac">FAC 300</span><span class="caps name">Hardware</span></h1>
		<p class="lede">What the devices can do on their own: firmware effects, the command dial, the screen.</p>
	</header>

	{#if !pipe.loaded}
		<p class="state">Asking uncoild…</p>
	{:else if pipe.unreachable}
		<div class="state" in:fade={{ duration: ms(260) }}>
			<p class="caps">uncoild isn't answering on its control pipe</p>
			<p>These controls need uncoild 0.2 or later running. Install it with <code>scripts\install-task.ps1</code>, then try again.</p>
			<button type="button" class="ghost caps" onclick={loadDevices}>Try again</button>
		</div>
	{:else}
		<div class="grid">
			{#if fxDevices.length}
				<section class="block fx" aria-labelledby="fx-title">
					<h2 id="fx-title" class="sub"><span class="num code">310</span><span class="caps">Firmware effect</span></h2>
					<Segmented label="Device" options={fxDevices.map((d, i) => ({ value: d.id, label: kindLabel(d.kind), code: `31${i + 1}` }))} value={fxId ?? ''} onchange={(v) => (fxId = v)} />
					<div class="fx-body">
						<CatalogList
							label="Effect"
							options={(fxCaps?.hw_effects ?? []).map((e, i) => ({ value: e, label: FX_LABEL[e] ?? e, code: String(i + 1).padStart(2, '0') }))}
							value={fxName}
							onchange={(v) => (fxName = v)}
						/>
						<div class="fx-params">
							{#if fxName === 'static' || fxName === 'reactive' || fxName === 'breathing' || fxName === 'starlight'}
								<div class="colors">
									<label class="swatch"><span class="caps-sm">Colour</span><input type="color" bind:value={fxColor} /></label>
									{#if fxName === 'breathing' || fxName === 'starlight'}
										<label class="swatch" class:off={!fxTwo}>
											<span class="caps-sm"><input type="checkbox" bind:checked={fxTwo} /> Second</span>
											<input type="color" bind:value={fxColor2} disabled={!fxTwo} />
										</label>
									{/if}
								</div>
							{:else if fxName === 'wave' || fxName === 'wheel'}
								<Segmented label="Direction" options={[{ value: 'left' as const, label: 'Left' }, { value: 'right' as const, label: 'Right' }]} value={fxDir} onchange={(v) => (fxDir = v)} />
								<Slider label="Speed" min={10} max={120} bind:value={fxSpeed} ends={['Fast', 'Slow']} format={(v) => String(Math.round(v))} hint="40 is the firmware's default." />
							{:else}
								<p class="note">No settings for this effect.</p>
							{/if}
							<p class="note">Runs on the device itself until you switch back or unplug it; nothing is saved. It can't span the desk the way uncoil's effect does.</p>
							<div class="row">
								<button type="button" class="btn" disabled={fxBusy || !fxCaps} onclick={() => runEffect(false)}>
									<span class="caps">Run on {fxDevice ? kindLabel(fxDevice.kind).toLowerCase() : 'device'}</span><span class="edge" aria-hidden="true"></span>
								</button>
								{#if fxDevice?.hw_effect}
									<button type="button" class="ghost caps" disabled={fxBusy} onclick={() => runEffect(true)} in:fade={{ duration: ms(200) }}>Back to uncoil</button>
								{/if}
							</div>
							{#if fxNote}<p class="outcome" class:bad={!fxNote.ok} role="status">{fxNote.text}</p>{/if}
						</div>
					</div>
				</section>
			{/if}

			{#if dialDevice}
				<section class="block" aria-labelledby="dial-title">
					<h2 id="dial-title" class="sub"><span class="num code">320</span><span class="caps">Command dial</span></h2>
					<CatalogList
						label="Dial mode"
						options={dialModes.map((m, i) => ({ value: m, label: DIAL_MODES[m] ?? m, code: String(i + 1).padStart(2, '0'), note: m === 'VOLUME' ? 'Tested' : undefined }))}
						value={dialChoice}
						current={dial?.mode ?? null}
						onchange={(v) => (dialChoice = v)}
					/>
					<p class="note">Volume is tested on hardware. The other modes are written exactly as Synapse writes them, but whether the keyboard acts on them without Synapse running hasn't been checked yet.</p>
					<WriteButton
						label="Save to keyboard"
						warning="Saves the dial mode in the keyboard's memory. Pick Volume and save again to undo."
						disabled={!dialChoice || dialChoice === dial?.mode}
						busy={dialBusy}
						onconfirm={saveDial}
					/>
					{#if dialNote}<p class="outcome" class:bad={!dialNote.ok} role="status">{dialNote.text}</p>{/if}
				</section>
			{/if}

			{#if oledDevice}
				<section class="block" aria-labelledby="oled-title">
					<h2 id="oled-title" class="sub"><span class="num code">330</span><span class="caps">Screen</span></h2>
					<Slider label="Brightness" min={0} max={100} bind:value={oledBright} format={(v) => `${Math.round(v)}%`} />
					<WriteButton
						label="Save brightness"
						warning="Saves the screen brightness in the keyboard's memory. It was {oled?.brightness ?? '?'}% before."
						disabled={!oled || Math.round(oledBright) === oled.brightness}
						busy={oledBusy}
						onconfirm={saveOled}
					/>
					{#if oledNote}<p class="outcome" class:bad={!oledNote.ok} role="status">{oledNote.text}</p>{/if}
					{#if oled}
						<dl class="readout">
							<div><dt class="caps-sm">Dims after</dt><dd class="num">{oled.time_to_dim_minutes ?? '—'} min</dd></div>
							<div><dt class="caps-sm">Home screen</dt><dd>{oled.home_screen ? oled.home_screen.toLowerCase() : '—'}</dd></div>
							<div><dt class="caps-sm">Shows</dt><dd>{oled.active_item ? oled.active_item.toLowerCase().replace(/_/g, ' ') : '—'}</dd></div>
							<div><dt class="caps-sm">Low power</dt><dd>{oled.low_power_mode == null ? '—' : oled.low_power_mode ? 'on' : 'off'}</dd></div>
						</dl>
						<p class="note">Only brightness can be changed so far; the rest is read from the keyboard.</p>
					{/if}
				</section>
			{/if}

			{#if profiles.length}
				<section class="block" aria-labelledby="prof-title">
					<h2 id="prof-title" class="sub"><span class="num code">340</span><span class="caps">Onboard profiles</span></h2>
					<table>
						<thead><tr class="caps-sm"><th scope="col">Device</th><th scope="col" class="r">In use</th><th scope="col" class="r">Slots</th><th scope="col" class="r">Active</th></tr></thead>
						<tbody>
							{#each profiles as p (p.name)}
								<tr>
									<td>{p.name}</td>
									{#if p.info}
										<td class="num r">{p.info.count}</td><td class="num r">{p.info.max}</td><td class="num r">{p.info.active ?? '—'}</td>
									{:else}
										<td colspan="3" class="r bad">{p.error}</td>
									{/if}
								</tr>
							{/each}
						</tbody>
					</table>
					<p class="note">Switching profiles from here isn't possible yet: the command for it hasn't been found. Key mappings are edited in profile 1.</p>
				</section>
			{/if}
		</div>
	{/if}
</section>

<style>
	.hw {
		display: grid;
		grid-template-rows: auto 1fr;
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
		max-width: 46ch;
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
	code {
		font-family: ui-monospace, 'Cascadia Mono', Consolas, monospace;
		font-size: 12px;
		color: var(--color-ink);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		align-content: start;
		min-height: 0;
		overflow: auto;
	}
	.block {
		display: grid;
		gap: 16px;
		align-content: start;
		padding: 22px 24px;
		border-bottom: var(--hair);
	}
	.block:nth-child(odd):not(.fx) {
		border-right: var(--hair);
	}
	.fx {
		grid-column: 1 / -1;
	}
	.fx ~ .block:nth-child(even) {
		border-right: var(--hair);
	}
	.fx ~ .block:nth-child(odd) {
		border-right: 0;
	}
	.sub {
		display: flex;
		align-items: baseline;
		gap: 12px;
		margin: 0;
		font-weight: 500;
		color: var(--color-ink-2);
	}
	.code {
		font-size: 12px;
		letter-spacing: 0.06em;
		color: var(--color-fac-red);
	}
	.fx-body {
		display: grid;
		grid-template-columns: minmax(0, 0.8fr) minmax(0, 1fr);
		gap: 28px;
		align-items: start;
	}
	.fx-params {
		display: grid;
		gap: 16px;
	}
	.colors {
		display: flex;
		gap: 18px;
	}
	.swatch {
		display: grid;
		gap: 8px;
		color: var(--color-ink-3);
		transition: opacity var(--t-mid) var(--ease);
	}
	.swatch.off {
		opacity: 0.6;
	}
	.swatch .caps-sm {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	input[type='checkbox'] {
		accent-color: var(--color-fac-red);
		margin: 0;
	}
	input[type='color'] {
		appearance: none;
		width: 64px;
		height: 30px;
		padding: 0;
		border: var(--hair-strong);
		background: none;
	}
	input[type='color']::-webkit-color-swatch-wrapper {
		padding: 3px;
	}
	input[type='color']::-webkit-color-swatch {
		border: 0;
		border-radius: 0;
	}
	.row {
		display: flex;
		gap: 10px;
		align-items: center;
	}
	.btn {
		position: relative;
		display: inline-flex;
		align-items: center;
		height: 38px;
		padding: 0 40px 0 16px;
		border: var(--hair-ink);
		background: none;
		color: var(--color-ink);
		overflow: hidden;
	}
	.btn:disabled {
		border-color: var(--color-seam-2);
		color: var(--color-ink-4);
	}
	.edge {
		position: absolute;
		right: 0;
		top: 0;
		bottom: 0;
		width: 26px;
		background: var(--color-fac-red);
		transform: scaleX(0.6923);
		transform-origin: right;
		transition: transform var(--t-mid) var(--ease);
	}
	.btn:hover:not(:disabled) .edge {
		transform: none;
	}
	.ghost {
		height: 38px;
		padding: 0 16px;
		border: var(--hair-strong);
		background: none;
		color: var(--color-ink-2);
		transition:
			color var(--t-mid) var(--ease),
			border-color var(--t-mid) var(--ease);
	}
	.ghost:hover {
		color: var(--color-ink);
		border-color: var(--color-ink-3);
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
		max-width: 60ch;
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
	.readout {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 12px 18px;
		margin: 0;
		padding-top: 14px;
		border-top: var(--hair);
	}
	.readout div {
		display: grid;
		gap: 3px;
	}
	dt {
		color: var(--color-ink-3);
	}
	dd {
		margin: 0;
		font-size: 13px;
		text-transform: capitalize;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th {
		text-align: left;
		color: var(--color-ink-3);
		font-weight: 500;
		padding: 0 10px 10px 0;
		border-bottom: var(--hair-strong);
	}
	td {
		padding: 12px 10px 12px 0;
		border-bottom: var(--hair);
		font-size: 13px;
	}
	.r {
		text-align: right;
	}
	.bad {
		color: var(--color-fac-yellow);
	}
	@container view (max-width: 820px) {
		.grid {
			grid-template-columns: minmax(0, 1fr);
		}
		.block {
			border-right: 0 !important;
		}
		.fx-body {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
