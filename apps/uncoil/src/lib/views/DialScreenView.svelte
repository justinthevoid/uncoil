<script lang="ts">
	import { onMount } from 'svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import OptionList from '#lib/components/OptionList.svelte';
	import Slider from '#lib/components/Slider.svelte';
	import WriteButton from '#lib/components/WriteButton.svelte';
	import PipeUnavailable from '#lib/components/PipeUnavailable.svelte';
	import { daemon } from '#lib/api.ts';
	import { pipe, loadDevices, errorText } from '#lib/daemon.svelte.ts';
	import { DIAL_MODES } from '#lib/keys.ts';
	import type { Capabilities, DialState, OledState, WriteResult } from '#lib/types.ts';

	let { deviceId }: { deviceId: string } = $props();
	type Note = { ok: boolean; text: string } | null;

	onMount(() => {
		if (!pipe.loaded) loadDevices();
	});
	const device = $derived(pipe.devices.find((d) => d.id === deviceId) ?? null);
	const hasDial = $derived(!!device?.features.includes('dial'));
	const hasOled = $derived(!!device?.features.includes('oled'));

	let dial = $state<DialState | null>(null);
	let dialModes = $state<string[]>([]);
	let dialChoice = $state<string | null>(null);
	let dialBusy = $state(false);
	let dialNote = $state<Note>(null);
	$effect(() => {
		if (!hasDial) return;
		Promise.all([daemon<DialState>('dial.get', deviceId), daemon<Capabilities>('capabilities', deviceId)])
			.then(([s, c]) => {
				dial = s;
				dialModes = c.dial_modes;
				dialChoice = s.mode;
			})
			.catch((e) => (dialNote = { ok: false, text: errorText(e) }));
	});
	async function saveDial() {
		if (!dialChoice) return;
		dialBusy = true;
		dialNote = null;
		try {
			const r = await daemon<WriteResult<DialState>>('dial.set', deviceId, { mode: dialChoice, write: true });
			dial = r.after;
			dialNote = r.unchanged
				? { ok: true, text: 'It was already in that mode.' }
				: { ok: r.verified, text: r.verified ? `Saved. The keyboard reads back ${DIAL_MODES[r.after.mode ?? ''] ?? r.after.mode}.` : 'Saved, but the keyboard reads back a different mode.' };
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
		if (!hasOled) return;
		daemon<OledState>('oled.get', deviceId)
			.then((s) => {
				oled = s;
				oledBright = s.brightness ?? 100;
			})
			.catch((e) => (oledNote = { ok: false, text: errorText(e) }));
	});
	async function saveOled() {
		oledBusy = true;
		oledNote = null;
		try {
			const r = await daemon<WriteResult<number>>('oled.set', deviceId, { brightness: Math.round(oledBright), write: true });
			if (oled) oled.brightness = r.after;
			oledNote = r.unchanged ? { ok: true, text: 'It was already at that brightness.' } : { ok: r.verified, text: `Saved. The screen reads back ${r.after}%.` };
		} catch (e) {
			oledNote = { ok: false, text: errorText(e) };
		} finally {
			oledBusy = false;
		}
	}
	const lower = (s: string | null) => (s ? s.toLowerCase().replace(/_/g, ' ').replace(/^\w/, (c) => c.toUpperCase()) : '—');
</script>

{#if !pipe.loaded}
	<p class="loading">Connecting to the engine…</p>
{:else if pipe.unreachable || !device}
	<PipeUnavailable what="The dial and screen" unreachable={pipe.unreachable} />
{:else}
	<Workspace title="Dial & screen" subtitle="Saved in the keyboard, so they work without uncoil.">
		<div class="grid">
			{#if hasDial}
				<section class="card" aria-labelledby="dial-title">
					<h2 id="dial-title" class="section-title">Dial</h2>
					<p class="note">What turning the dial does.</p>
					<OptionList
						label="Dial mode"
						options={dialModes.map((m) => ({ value: m, label: DIAL_MODES[m] ?? m, note: m === 'VOLUME' ? 'Tested' : undefined }))}
						value={dialChoice}
						current={dial?.mode ?? null}
						onchange={(v) => (dialChoice = v)}
					/>
					<p class="note">Volume is tested. The others are sent exactly as Synapse sends them; whether the keyboard acts on them without Synapse hasn't been checked yet.</p>
					<WriteButton label="Save to keyboard" warning="Saves the dial mode in the keyboard's memory. Choose Volume and save again to undo." disabled={!dialChoice || dialChoice === dial?.mode} busy={dialBusy} onconfirm={saveDial} />
					{#if dialNote}<p class="outcome" class:bad={!dialNote.ok} role="status">{dialNote.text}</p>{/if}
				</section>
			{/if}
			{#if hasOled}
				<section class="card" aria-labelledby="oled-title">
					<h2 id="oled-title" class="section-title">Screen</h2>
					<p class="note">The keyboard's small display.</p>
					<Slider label="Brightness" min={0} max={100} bind:value={oledBright} format={(v) => `${Math.round(v)}%`} />
					<WriteButton label="Save brightness" warning="Saves the screen brightness in the keyboard's memory. It was {oled?.brightness ?? '?'}% before." disabled={!oled || Math.round(oledBright) === oled.brightness} busy={oledBusy} onconfirm={saveOled} />
					{#if oledNote}<p class="outcome" class:bad={!oledNote.ok} role="status">{oledNote.text}</p>{/if}
					{#if oled}
						<dl class="readout">
							<div><dt>Dims after</dt><dd class="num">{oled.time_to_dim_minutes ?? '—'} min</dd></div>
							<div><dt>Home screen</dt><dd>{lower(oled.home_screen)}</dd></div>
							<div><dt>Shows</dt><dd>{lower(oled.active_item)}</dd></div>
							<div><dt>Low power mode</dt><dd>{oled.low_power_mode == null ? '—' : oled.low_power_mode ? 'On' : 'Off'}</dd></div>
						</dl>
						<p class="note">Only brightness can be changed so far; the rest is read from the keyboard.</p>
					{/if}
				</section>
			{/if}
			{#if !hasDial && !hasOled}
				<p class="note">This device has no dial or screen.</p>
			{/if}
		</div>
	</Workspace>
{/if}

<style>
	.loading {
		margin: 24px 28px;
		color: var(--color-ink-3);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 16px;
		align-items: start;
		max-width: 980px;
	}
	.card {
		display: grid;
		gap: 12px;
		padding: 18px;
		border: var(--hair);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
	}
	.readout {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 12px 18px;
		margin: 4px 0 0;
		padding-top: 14px;
		border-top: var(--hair);
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
		font-weight: 500;
	}
	@container view (max-width: 640px) {
		.grid {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
