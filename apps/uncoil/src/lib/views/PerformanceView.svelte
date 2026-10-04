<script lang="ts">
	// DPI and polling rate. The current DPI is live (sent as you drag, never stored, like pressing the mouse's
	// DPI button). DPI stages and the polling rate are stored in the mouse, so they go through the two-step
	// onboard write and report what the mouse read back.
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { Plus, X } from '@lucide/svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import Slider from '#lib/components/Slider.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import WriteButton from '#lib/components/WriteButton.svelte';
	import CheckNotice from '#lib/components/CheckNotice.svelte';
	import PipeUnavailable from '#lib/components/PipeUnavailable.svelte';
	import { daemon } from '#lib/api.ts';
	import { pipe, loadDevices, errorText, experimentalBadge } from '#lib/daemon.svelte.ts';
	import { locked } from '#lib/checks.ts';
	import { ms } from '#lib/motion.ts';
	import type { Capabilities, Dpi, Feature, FeatureCheck, PerformanceState, WriteResult } from '#lib/types.ts';

	let { deviceId }: { deviceId: string } = $props();

	onMount(() => {
		if (!pipe.loaded) loadDevices();
	});
	const device = $derived(pipe.devices.find((d) => d.id === deviceId) ?? null);

	let caps = $state<Capabilities | null>(null);
	let perf = $state<PerformanceState | null>(null);
	let loadError = $state<string | null>(null);

	// Drafts, reset from what the mouse reports.
	let dpi = $state(800);
	let stages = $state<Dpi[]>([]);
	let active = $state(1);
	let linked = $state(true);
	let poll = $state<number>(1000);

	function adopt(p: PerformanceState, parts: { live?: boolean; stages?: boolean; poll?: boolean } = { live: true, stages: true, poll: true }) {
		perf = p;
		if (parts.live && p.dpi) dpi = p.dpi.x;
		if (parts.stages && p.stages) {
			stages = p.stages.list.map((s) => ({ ...s }));
			active = p.stages.active;
			linked = p.stages.list.every((s) => s.x === s.y);
		}
		if (parts.poll && p.poll_hz !== null) poll = p.poll_hz;
	}

	$effect(() => {
		if (!device) return;
		loadError = null;
		Promise.all([daemon<Capabilities>('capabilities', deviceId), daemon<PerformanceState>('performance.get', deviceId)])
			.then(([c, p]) => {
				caps = c;
				adopt(p);
			})
			.catch((e) => (loadError = errorText(e)));
	});

	const features = $derived((['dpi', 'poll_rate'] as Feature[]).filter((f) => device?.features.includes(f)));
	const dpiLocked = $derived(locked(caps, 'dpi'));
	const pollLocked = $derived(locked(caps, 'poll_rate'));
	const onchecks = (checks: FeatureCheck[]) => caps && (caps = { ...caps, checks });

	const min = $derived(perf?.dpi_min ?? 100);
	const max = $derived(perf?.dpi_max ?? 30000);
	const clamp = (v: number) => Math.min(max, Math.max(min, Math.round(v)));

	// ---- live DPI: sent ~120 ms after the last change, no confirm ----------------------------------------
	let liveTimer: ReturnType<typeof setTimeout> | undefined;
	let liveError = $state<string | null>(null);
	$effect(() => {
		const v = dpi;
		if (!perf?.dpi || dpiLocked || v === perf.dpi.x) return;
		clearTimeout(liveTimer);
		liveTimer = setTimeout(async () => {
			try {
				const p = await daemon<PerformanceState>('performance.set', deviceId, { dpi: { x: v, y: v } });
				if (perf) perf.dpi = p.dpi;
				liveError = null;
			} catch (e) {
				liveError = errorText(e);
			}
		}, 120);
	});
	function typedDpi(e: Event) {
		const el = e.currentTarget as HTMLInputElement;
		const v = Number(el.value);
		if (Number.isFinite(v) && el.value !== '') dpi = clamp(v);
		el.value = String(dpi);
	}

	// ---- stages -------------------------------------------------------------------------------------------
	const stagesMax = $derived(perf?.stages_max ?? 0);
	const valid = (v: number) => Number.isFinite(v) && v >= min && v <= max;
	const stagesValid = $derived(stages.length > 0 && stages.every((s) => valid(s.x) && valid(s.y)));
	const stagesDirty = $derived(!!perf?.stages && (active !== perf.stages.active || JSON.stringify(stages) !== JSON.stringify(perf.stages.list)));
	function setStage(i: number, axis: 'x' | 'y' | 'both', raw: string) {
		const v = raw === '' ? NaN : Math.round(Number(raw));
		stages = stages.map((s, j) => (j !== i ? s : axis === 'both' ? { x: v, y: v } : { ...s, [axis]: v }));
	}
	function addStage() {
		const last = stages[stages.length - 1]?.x ?? 800;
		const v = clamp(Number.isFinite(last) ? last * 2 : 800);
		stages = [...stages, { x: v, y: v }];
	}
	function removeStage(i: number) {
		stages = stages.filter((_, j) => j !== i);
		if (active > i + 1 || active > stages.length) active = Math.max(1, active - 1);
	}
	$effect(() => {
		// Linking again copies each stage's X value to Y.
		if (linked) {
			if (stages.some((s) => s.x !== s.y)) stages = stages.map((s) => ({ x: s.x, y: s.x }));
		}
	});

	const fmt = (d: Dpi) => (d.x === d.y ? `${d.x}` : `${d.x}×${d.y}`);
	type Outcome = { ok: boolean; text: string } | null;
	let stagesBusy = $state(false);
	let stagesOutcome = $state<Outcome>(null);
	async function saveStages() {
		stagesBusy = true;
		stagesOutcome = null;
		try {
			const r = await daemon<WriteResult<PerformanceState>>('performance.set', deviceId, { stages: { active, list: $state.snapshot(stages) }, write: true });
			adopt(r.after, { live: true, stages: true });
			const st = r.after.stages;
			const back = st ? `${st.list.map(fmt).join(', ')} DPI, stage ${st.active} active` : 'no stages';
			stagesOutcome = r.unchanged ? { ok: true, text: 'It was already set that way.' } : { ok: r.verified, text: `${r.verified ? 'Saved.' : 'Saved, but'} The mouse reads back ${back}.` };
		} catch (e) {
			stagesOutcome = { ok: false, text: errorText(e) };
		} finally {
			stagesBusy = false;
		}
	}

	// ---- polling rate ---------------------------------------------------------------------------------
	const pollOptions = $derived((perf?.poll_rates ?? []).map((hz) => ({ value: String(hz), label: String(hz) })));
	const pollDirty = $derived(perf?.poll_hz !== null && perf?.poll_hz !== undefined && poll !== perf.poll_hz);
	let pollBusy = $state(false);
	let pollOutcome = $state<Outcome>(null);
	async function savePoll() {
		pollBusy = true;
		pollOutcome = null;
		try {
			const r = await daemon<WriteResult<PerformanceState>>('performance.set', deviceId, { poll_hz: poll, write: true });
			adopt(r.after, { poll: true });
			pollOutcome = r.unchanged
				? { ok: true, text: 'It was already set that way.' }
				: { ok: r.verified, text: `${r.verified ? 'Saved.' : 'Saved, but'} The mouse reads back ${r.after.poll_hz ?? 'nothing'} Hz.` };
		} catch (e) {
			pollOutcome = { ok: false, text: errorText(e) };
		} finally {
			pollBusy = false;
		}
	}
</script>

{#if !pipe.loaded}
	<p class="loading">Connecting to the engine…</p>
{:else if pipe.unreachable || !device}
	<PipeUnavailable page="performance" unreachable={pipe.unreachable} />
{:else}
	<Workspace
		title={pageTitle('performance')}
		badge={experimentalBadge(deviceId)}
		subtitle="The current DPI changes right away and isn't stored. DPI stages and the polling rate are saved in the mouse, so they keep working without uncoil."
	>
		{#if loadError}
			<p class="outcome bad" role="alert">{loadError}</p>
		{:else if perf}
			<CheckNotice {deviceId} {caps} {features} {onchecks} />
			<div class="grid">
				{#if perf.dpi}
					<section class="card live" aria-labelledby="live-title">
						<h2 id="live-title" class="section-title">Current DPI</h2>
						<fieldset disabled={dpiLocked}>
							<div class="dpi-row">
								<div class="slider"><Slider label="DPI" bind:value={dpi} {min} {max} step={50} ends={['Slower pointer', 'Faster pointer']} disabled={dpiLocked} /></div>
								<input class="input num" type="number" aria-label="DPI" {min} {max} step="50" value={dpi} onchange={typedDpi} />
							</div>
						</fieldset>
						<p class="hint">Changes as you drag. It isn't saved: pressing the mouse's DPI button or reconnecting it goes back to a stage.</p>
						{#if liveError}<p class="outcome bad" role="alert">{liveError}</p>{/if}
					</section>
				{/if}

				{#if pollOptions.length}
					<section class="card poll" aria-labelledby="poll-title">
						<h2 id="poll-title" class="section-title">Polling rate</h2>
						<p class="hint">How many times a second (Hz) the mouse reports its position to the PC. Higher is smoother but uses more CPU{device.connection !== 'wired' ? ' and battery' : ''}.</p>
						<fieldset disabled={pollLocked}>
							<Segmented label="Polling rate" options={pollOptions} value={String(poll)} onchange={(v) => (poll = Number(v))} />
						</fieldset>
						<WriteButton label="Save to mouse" warning="The polling rate is saved in the mouse's own memory. Pick another rate and save again to change it back." disabled={!pollDirty || pollLocked} busy={pollBusy} onconfirm={savePoll} />
						{#if pollOutcome}<p class="outcome" class:bad={!pollOutcome.ok} role="status" in:fade={{ duration: ms(160) }}>{pollOutcome.text}</p>{/if}
					</section>
				{/if}

				{#if perf.stages && stagesMax > 0}
					<section class="card stages" aria-labelledby="stages-title">
						<h2 id="stages-title" class="section-title">DPI stages</h2>
						<p class="hint">The mouse's DPI button steps through these. The chosen stage is the one it starts on.</p>
						<fieldset disabled={dpiLocked}>
							<Toggle label="Same DPI for X and Y" bind:checked={linked} />
							<div class="rows" role="radiogroup" aria-label="Active stage">
								<div class="row head" class:split={!linked} aria-hidden="true">
									<span></span><span>{linked ? 'DPI' : 'X'}</span>{#if !linked}<span>Y</span>{/if}<span></span>
								</div>
								{#each stages as s, i (i)}
									<div class="row" class:split={!linked}>
										<button type="button" class="stage" role="radio" aria-checked={active === i + 1} onclick={() => (active = i + 1)}>
											<span class="dot" aria-hidden="true"></span>Stage {i + 1}
										</button>
										{#if linked}
											<input class="input num" class:bad={!valid(s.x)} type="number" aria-label="Stage {i + 1} DPI" {min} {max} step="50" value={Number.isFinite(s.x) ? s.x : ''} oninput={(e) => setStage(i, 'both', e.currentTarget.value)} />
										{:else}
											<input class="input num" class:bad={!valid(s.x)} type="number" aria-label="Stage {i + 1} DPI X" {min} {max} step="50" value={Number.isFinite(s.x) ? s.x : ''} oninput={(e) => setStage(i, 'x', e.currentTarget.value)} />
											<input class="input num" class:bad={!valid(s.y)} type="number" aria-label="Stage {i + 1} DPI Y" {min} {max} step="50" value={Number.isFinite(s.y) ? s.y : ''} oninput={(e) => setStage(i, 'y', e.currentTarget.value)} />
										{/if}
										<button type="button" class="remove" aria-label="Remove stage {i + 1}" title="Remove stage" disabled={stages.length <= 1} onclick={() => removeStage(i)}><X size={15} /></button>
									</div>
								{/each}
							</div>
							<div class="stage-tools">
								<button type="button" class="btn-quiet" disabled={stages.length >= stagesMax} onclick={addStage}><Plus size={15} />Add stage</button>
								<span class="hint">{stages.length} of {stagesMax}{stagesValid ? '' : ` · each between ${min} and ${max}`}</span>
							</div>
						</fieldset>
						<WriteButton
							label="Save to mouse"
							warning="The stages are saved in the mouse's own memory, and the mouse switches to the chosen stage. Change them and save again to undo."
							disabled={!stagesDirty || !stagesValid || dpiLocked}
							busy={stagesBusy}
							onconfirm={saveStages}
						/>
						{#if stagesOutcome}<p class="outcome" class:bad={!stagesOutcome.ok} role="status" in:fade={{ duration: ms(160) }}>{stagesOutcome.text}</p>{/if}
					</section>
				{/if}
			</div>
		{:else}
			<p class="loading-inline">Reading the mouse…</p>
		{/if}
	</Workspace>
{/if}

<style>
	.loading {
		margin: 24px 28px;
		color: var(--color-ink-3);
	}
	.loading-inline {
		margin: 0;
		color: var(--color-ink-3);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		grid-template-areas: 'live stages' 'poll stages';
		grid-template-rows: auto 1fr;
		gap: 16px;
		align-items: start;
		max-width: 980px;
	}
	.live {
		grid-area: live;
	}
	.poll {
		grid-area: poll;
	}
	.stages {
		grid-area: stages;
	}
	.card {
		display: grid;
		gap: 12px;
		padding: 18px;
		border: var(--hair);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		min-width: 0;
	}
	fieldset {
		display: grid;
		gap: 12px;
		min-width: 0;
		margin: 0;
		padding: 0;
		border: 0;
		transition: opacity var(--t-mid) var(--ease);
	}
	fieldset:disabled {
		opacity: 0.45;
	}
	.hint {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
	}
	.dpi-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 92px;
		gap: 14px;
		align-items: center;
	}
	.dpi-row :global(.slider.disabled) {
		opacity: 1;
	}
	.input {
		width: 100%;
		min-width: 0;
		text-align: right;
	}
	.input.bad {
		border-color: var(--color-warn);
	}
	.rows {
		display: grid;
		gap: 6px;
	}
	.row {
		display: grid;
		grid-template-columns: minmax(96px, 1fr) minmax(0, 110px) 30px;
		gap: 8px;
		align-items: center;
	}
	.row.split {
		grid-template-columns: minmax(96px, 1fr) minmax(0, 96px) minmax(0, 96px) 30px;
	}
	.row.head {
		color: var(--color-ink-3);
		font-size: 12px;
		font-weight: 500;
	}
	.row.head span:nth-child(n + 2) {
		text-align: right;
		padding-right: 10px;
	}
	.stage {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 34px;
		padding: 0 10px;
		border: 0;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-2);
		text-align: left;
		transition: background-color var(--t-mid) var(--ease);
	}
	.stage:hover:not(:disabled) {
		color: var(--color-ink);
	}
	.stage[aria-checked='true'] {
		background: var(--color-surface-2);
		color: var(--color-ink);
		font-weight: 600;
	}
	.dot {
		width: 14px;
		height: 14px;
		flex: none;
		border-radius: 50%;
		border: 1.5px solid var(--color-ink-4);
	}
	.stage[aria-checked='true'] .dot {
		border: 4.5px solid var(--color-select);
	}
	.remove {
		display: grid;
		place-items: center;
		width: 30px;
		height: 30px;
		border: 0;
		border-radius: var(--radius);
		background: none;
		color: var(--color-ink-3);
	}
	.remove:hover:not(:disabled) {
		background: var(--color-surface-2);
		color: var(--color-ink);
	}
	.remove:disabled {
		opacity: 0.3;
	}
	.stage-tools {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.poll :global(.seg button) {
		padding: 0 4px;
	}
	.card :global(.write:has(.confirm)) {
		width: 100%;
	}
	@container view (max-width: 760px) {
		.grid {
			grid-template-columns: minmax(0, 1fr);
			grid-template-areas: 'live' 'poll' 'stages';
			grid-template-rows: auto;
		}
	}
</style>
