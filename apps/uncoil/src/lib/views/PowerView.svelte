<script lang="ts">
	// Battery level and charging (read-only), plus the sleep timer and low battery warning, which are stored
	// in the mouse and go through the two-step onboard write.
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import Slider from '#lib/components/Slider.svelte';
	import WriteButton from '#lib/components/WriteButton.svelte';
	import CheckNotice from '#lib/components/CheckNotice.svelte';
	import PipeUnavailable from '#lib/components/PipeUnavailable.svelte';
	import { daemon } from '#lib/api.ts';
	import { pipe, loadDevices, errorText, experimentalBadge } from '#lib/daemon.svelte.ts';
	import { locked } from '#lib/checks.ts';
	import { ms } from '#lib/motion.ts';
	import type { Capabilities, FeatureCheck, PowerState, WriteResult } from '#lib/types.ts';

	let { deviceId }: { deviceId: string } = $props();

	onMount(() => {
		if (!pipe.loaded) loadDevices();
	});
	const device = $derived(pipe.devices.find((d) => d.id === deviceId) ?? null);

	let caps = $state<Capabilities | null>(null);
	let power = $state<PowerState | null>(null);
	let loadError = $state<string | null>(null);
	let idle = $state(300);
	let low = $state(15);

	function adopt(p: PowerState) {
		power = p;
		if (p.idle_s !== null) idle = p.idle_s;
		if (p.low_battery_pct !== null) low = p.low_battery_pct;
	}

	$effect(() => {
		if (!device) return;
		loadError = null;
		Promise.all([daemon<Capabilities>('capabilities', deviceId), daemon<PowerState>('power.get', deviceId)])
			.then(([c, p]) => {
				caps = c;
				adopt(p);
			})
			.catch((e) => (loadError = errorText(e)));
	});

	// The battery level moves; read it again every 30 seconds while this page is open.
	onMount(() => {
		const t = setInterval(async () => {
			if (!power) return;
			try {
				const p = await daemon<PowerState>('power.get', deviceId);
				if (power) {
					power.battery_pct = p.battery_pct;
					power.charging = p.charging;
				}
			} catch {
				/* keep the last reading */
			}
		}, 30_000);
		return () => clearInterval(t);
	});

	const isLocked = $derived(locked(caps, 'power'));
	const onchecks = (checks: FeatureCheck[]) => caps && (caps = { ...caps, checks });

	const duration = (s: number) => {
		const m = Math.floor(s / 60);
		const r = s % 60;
		if (!m) return `${r} s`;
		return r ? `${m} min ${r} s` : `${m} min`;
	};
	const pct = (v: number) => `${Math.round(v)}%`;

	const dirty = $derived(!!power && ((power.idle_s !== null && idle !== power.idle_s) || (power.low_battery_pct !== null && low !== power.low_battery_pct)));
	let busy = $state(false);
	let outcome = $state<{ ok: boolean; text: string } | null>(null);
	async function save() {
		if (!power) return;
		busy = true;
		outcome = null;
		const args: Record<string, unknown> = { write: true };
		if (power.idle_s !== null && idle !== power.idle_s) args.idle_s = idle;
		if (power.low_battery_pct !== null && low !== power.low_battery_pct) args.low_battery_pct = low;
		try {
			const r = await daemon<WriteResult<PowerState>>('power.set', deviceId, args);
			adopt(r.after);
			const back = [r.after.idle_s !== null ? `sleeps after ${duration(r.after.idle_s)}` : '', r.after.low_battery_pct !== null ? `warns at ${pct(r.after.low_battery_pct)}` : '']
				.filter(Boolean)
				.join(', ');
			outcome = r.unchanged ? { ok: true, text: 'It was already set that way.' } : { ok: r.verified, text: `${r.verified ? 'Saved.' : 'Saved, but'} The mouse reads back: ${back}.` };
		} catch (e) {
			outcome = { ok: false, text: errorText(e) };
		} finally {
			busy = false;
		}
	}
</script>

{#if !pipe.loaded}
	<p class="loading">Connecting to the engine…</p>
{:else if pipe.unreachable || !device}
	<PipeUnavailable page="power" unreachable={pipe.unreachable} />
{:else}
	<Workspace title={pageTitle('power')} badge={experimentalBadge(deviceId)} subtitle="The sleep timer and low battery warning are saved in the mouse, so they keep working without uncoil.">
		{#if loadError}
			<p class="outcome bad" role="alert">{loadError}</p>
		{:else if power}
			<CheckNotice {deviceId} {caps} features={['power']} {onchecks} />
			<div class="grid">
				<section class="card" aria-labelledby="battery-title">
					<h2 id="battery-title" class="section-title">Battery</h2>
					{#if power.battery_pct !== null}
						<p class="level"><span class="num">{power.battery_pct}%</span><span class="state">{power.charging === null ? '' : power.charging ? 'Charging' : 'Not charging'}</span></p>
						<div class="bar" role="meter" aria-label="Battery level" aria-valuemin={0} aria-valuemax={100} aria-valuenow={power.battery_pct}>
							<span style:width="{power.battery_pct}%"></span>
						</div>
						{#if power.low_battery_pct !== null}
							<p class="hint">The mouse warns you when it drops to {pct(power.low_battery_pct)}.</p>
						{/if}
					{:else}
						<p class="hint">This mouse doesn't report its battery level.</p>
					{/if}
				</section>

				<section class="card" aria-labelledby="sleep-title">
					<h2 id="sleep-title" class="section-title">Sleep and warning</h2>
					<fieldset disabled={isLocked}>
						{#if power.idle_s !== null && power.idle_range}
							<Slider
								label="Sleep after"
								bind:value={idle}
								min={power.idle_range[0]}
								max={power.idle_range[1]}
								step={30}
								format={duration}
								ends={[duration(power.idle_range[0]), duration(power.idle_range[1])]}
								hint="How long the mouse waits without moving before it goes to sleep to save battery."
								disabled={isLocked}
							/>
						{/if}
						{#if power.low_battery_pct !== null && power.low_battery_range}
							<Slider
								label="Low battery warning"
								bind:value={low}
								min={power.low_battery_range[0]}
								max={power.low_battery_range[1]}
								step={1}
								format={pct}
								ends={[pct(power.low_battery_range[0]), pct(power.low_battery_range[1])]}
								hint="The mouse shows its low battery warning when the battery drops to this level."
								disabled={isLocked}
							/>
						{/if}
					</fieldset>
					<WriteButton label="Save to mouse" warning="These are saved in the mouse's own memory. Move the sliders back and save again to undo." disabled={!dirty || isLocked} {busy} onconfirm={save} />
					{#if outcome}<p class="outcome" class:bad={!outcome.ok} role="status" in:fade={{ duration: ms(160) }}>{outcome.text}</p>{/if}
				</section>
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
		grid-template-columns: minmax(0, 0.8fr) minmax(0, 1fr);
		gap: 16px;
		align-items: start;
		max-width: 900px;
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
		gap: 20px;
		min-width: 0;
		margin: 0;
		padding: 0;
		border: 0;
		transition: opacity var(--t-mid) var(--ease);
	}
	fieldset:disabled {
		opacity: 0.45;
	}
	fieldset :global(.slider.disabled) {
		opacity: 1;
	}
	.level {
		display: flex;
		align-items: baseline;
		gap: 10px;
		margin: 0;
	}
	.level .num {
		font-family: var(--font-display);
		font-size: 28px;
		font-weight: 600;
		letter-spacing: -0.01em;
	}
	.state {
		color: var(--color-ink-3);
	}
	.bar {
		height: 8px;
		border-radius: 4px;
		background: var(--color-surface-3);
		overflow: hidden;
	}
	.bar span {
		display: block;
		height: 100%;
		border-radius: 4px;
		background: var(--color-ink-2);
	}
	.hint {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
	}
	.card :global(.write:has(.confirm)) {
		width: 100%;
	}
	@container view (max-width: 680px) {
		.grid {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
