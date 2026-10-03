<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import Segmented from '#lib/components/Segmented.svelte';
	import OptionList from '#lib/components/OptionList.svelte';
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
			fxNote = { ok: true, text: fx ? `The ${kindLabel(fxDevice!.kind).toLowerCase()} is running its built-in ${fx} effect.` : 'Back on uncoil’s effect.' };
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
		<h1 id="hw-title" class="page-title">Dial &amp; screen</h1>
		<p class="lede">Settings stored in the devices themselves: the keyboard's dial and screen, effects the devices can run on their own, and onboard profiles.</p>
	</header>

	{#if !pipe.loaded}
		<p class="state">Connecting to the engine…</p>
	{:else if pipe.unreachable}
		<div class="state panel" in:fade={{ duration: ms(260) }}>
			<p class="section-title">The engine isn't answering</p>
			<p>These settings need the latest uncoild running. Install it with <code>scripts\install-task.ps1</code>, then try again.</p>
			<button type="button" class="btn-quiet" onclick={loadDevices}>Try again</button>
		</div>
	{:else}
		<div class="grid">
			{#if dialDevice}
				<section class="card" aria-labelledby="dial-title">
					<h2 id="dial-title" class="section-title">Dial</h2>
					<p class="note">What turning the keyboard's dial does.</p>
					<OptionList
						label="Dial mode"
						options={dialModes.map((m) => ({ value: m, label: DIAL_MODES[m] ?? m, note: m === 'VOLUME' ? 'Tested' : undefined }))}
						value={dialChoice}
						current={dial?.mode ?? null}
						onchange={(v) => (dialChoice = v)}
					/>
					<p class="note">Volume is tested. The others are sent exactly as Synapse sends them, but whether the keyboard acts on them without Synapse hasn't been checked yet.</p>
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
				<section class="card" aria-labelledby="oled-title">
					<h2 id="oled-title" class="section-title">Screen</h2>
					<p class="note">The keyboard's small display.</p>
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
							<div><dt>Dims after</dt><dd class="num">{oled.time_to_dim_minutes ?? '—'} min</dd></div>
							<div><dt>Home screen</dt><dd>{oled.home_screen ? oled.home_screen.toLowerCase() : '—'}</dd></div>
							<div><dt>Shows</dt><dd>{oled.active_item ? oled.active_item.toLowerCase().replace(/_/g, ' ') : '—'}</dd></div>
							<div><dt>Low power mode</dt><dd>{oled.low_power_mode == null ? '—' : oled.low_power_mode ? 'on' : 'off'}</dd></div>
						</dl>
						<p class="note">Only brightness can be changed so far.</p>
					{/if}
				</section>
			{/if}

			{#if fxDevices.length}
				<section class="card wide" aria-labelledby="fx-title">
					<h2 id="fx-title" class="section-title">Onboard effects</h2>
					<p class="note">Let a device run one of its built-in effects by itself: no CPU at all, but it won't flow across the desk like uncoil's effect. Nothing is saved; unplugging or switching back ends it.</p>
					<div class="seg"><Segmented label="Device" options={fxDevices.map((d) => ({ value: d.id, label: kindLabel(d.kind) }))} value={fxId ?? ''} onchange={(v) => (fxId = v)} /></div>
					<div class="fx-body">
						<OptionList label="Effect" options={(fxCaps?.hw_effects ?? []).map((e) => ({ value: e, label: FX_LABEL[e] ?? e }))} value={fxName} onchange={(v) => (fxName = v)} />
						<div class="fx-params">
							{#if fxName === 'static' || fxName === 'reactive' || fxName === 'breathing' || fxName === 'starlight'}
								<div class="colors">
									<label class="swatch"><span class="label">Colour</span><input type="color" bind:value={fxColor} /></label>
									{#if fxName === 'breathing' || fxName === 'starlight'}
										<label class="swatch" class:off={!fxTwo}>
											<span class="label"><input type="checkbox" bind:checked={fxTwo} /> Second colour</span>
											<input type="color" bind:value={fxColor2} disabled={!fxTwo} />
										</label>
									{/if}
								</div>
							{:else if fxName === 'wave' || fxName === 'wheel'}
								<Segmented label="Direction" options={[{ value: 'left' as const, label: 'Left' }, { value: 'right' as const, label: 'Right' }]} value={fxDir} onchange={(v) => (fxDir = v)} />
								<Slider label="Speed" min={10} max={120} bind:value={fxSpeed} ends={['Fast', 'Slow']} format={(v) => String(Math.round(v))} hint="40 is the device's default." />
							{:else}
								<p class="note">This effect has no settings.</p>
							{/if}
							<div class="row">
								<button type="button" class="btn" disabled={fxBusy || !fxCaps} onclick={() => runEffect(false)}>
									Run on {fxDevice ? kindLabel(fxDevice.kind).toLowerCase() : 'device'}
								</button>
								{#if fxDevice?.hw_effect}
									<button type="button" class="btn-quiet" disabled={fxBusy} onclick={() => runEffect(true)} in:fade={{ duration: ms(200) }}>Back to uncoil's effect</button>
								{/if}
							</div>
							{#if fxNote}<p class="outcome" class:bad={!fxNote.ok} role="status">{fxNote.text}</p>{/if}
						</div>
					</div>
				</section>
			{/if}

			{#if profiles.length}
				<section class="card wide" aria-labelledby="prof-title">
					<h2 id="prof-title" class="section-title">Onboard profiles</h2>
					<table>
						<thead><tr><th scope="col">Device</th><th scope="col" class="r">Profiles in use</th><th scope="col" class="r">Slots</th><th scope="col" class="r">Active</th></tr></thead>
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
					<p class="note">Switching profiles from here isn't possible yet. Key changes apply to profile 1.</p>
				</section>
			{/if}
		</div>
	{/if}
</section>

<style>
	.hw {
		display: grid;
		grid-template-rows: auto 1fr;
		gap: 16px;
		min-height: 0;
	}
	.lede {
		margin: 6px 0 0;
		color: var(--color-ink-3);
		font-size: 13px;
		max-width: 75ch;
	}
	.state {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 13px;
	}
	.panel,
	.card {
		display: grid;
		gap: 12px;
		align-content: start;
		padding: 18px 20px;
		border-radius: var(--radius-lg);
		background: var(--color-raised);
		border: 1px solid var(--color-seam);
	}
	.panel {
		justify-items: start;
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
	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 16px;
		align-content: start;
	}
	.wide {
		grid-column: 1 / -1;
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
		max-width: 70ch;
	}
	.seg {
		max-width: 340px;
	}
	.fx-body {
		display: grid;
		grid-template-columns: minmax(0, 0.7fr) minmax(0, 1fr);
		gap: 24px;
		align-items: start;
	}
	.fx-params {
		display: grid;
		gap: 14px;
	}
	.colors {
		display: flex;
		gap: 18px;
	}
	.swatch {
		display: grid;
		gap: 8px;
		transition: opacity var(--t-mid) var(--ease);
	}
	.swatch.off {
		opacity: 0.6;
	}
	.swatch .label {
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
		width: 56px;
		height: 32px;
		padding: 0;
		border: 1px solid var(--color-seam-2);
		border-radius: var(--radius);
		background: none;
		overflow: hidden;
	}
	input[type='color']::-webkit-color-swatch-wrapper {
		padding: 3px;
	}
	input[type='color']::-webkit-color-swatch {
		border: 0;
		border-radius: var(--radius-sm);
	}
	.row {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}
	.readout {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 12px 18px;
		margin: 4px 0 0;
		padding-top: 14px;
		border-top: 1px solid var(--color-seam);
	}
	.readout div {
		display: grid;
		gap: 2px;
	}
	dt {
		color: var(--color-ink-3);
		font-size: 12px;
	}
	dd {
		margin: 0;
		font-size: 13px;
	}
	dd::first-letter {
		text-transform: uppercase;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th {
		text-align: left;
		color: var(--color-ink-3);
		font-size: 12px;
		font-weight: 500;
		padding: 0 10px 8px 0;
		border-bottom: 1px solid var(--color-seam-2);
	}
	td {
		padding: 10px 10px 10px 0;
		border-bottom: 1px solid var(--color-seam);
		font-size: 13px;
	}
	.r {
		text-align: right;
	}
	.bad {
		color: var(--color-fac-yellow);
	}
	@container view (max-width: 820px) {
		.grid,
		.fx-body {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
