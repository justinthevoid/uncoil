<script lang="ts">
	import { ExternalLink } from '@lucide/svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import { pageTitle } from '#lib/pages.ts';
	import Slider from '#lib/components/Slider.svelte';
	import Toggle from '#lib/components/Toggle.svelte';
	import OptionList from '#lib/components/OptionList.svelte';
	import { openExternal } from '#lib/api.ts';
	import { app } from '#lib/state.svelte.ts';
	import { CONFIG_DOC_URL } from '#lib/checks.ts';
	import type { Config, OpenRgbMode } from '#lib/types.ts';

	let { config }: { config: Config } = $props();
	const d = $derived(config.display);
	const now = $derived(app.status?.display ?? null);

	// ---- OpenRGB: the PC's other lighting -----------------------------------------------------------
	const MODES: { value: OpenRgbMode; label: string }[] = [
		{ value: 'off', label: 'Off' },
		{ value: 'hardware', label: 'Hand off once at sign-in' },
		{ value: 'live', label: 'Live: follow the desk effect' }
	];
	const mode = $derived<OpenRgbMode>(config.openrgb?.mode ?? (config.openrgb_hardware_rainbow ? 'hardware' : 'off'));
	function setMode(m: OpenRgbMode) {
		config.openrgb = { ...(config.openrgb ?? { devices: [] }), mode: m };
		// Older engines read only the flag; keep it in step while the config still carries it.
		if (config.openrgb_hardware_rainbow !== undefined) config.openrgb_hardware_rainbow = m === 'hardware';
	}
	const listed = $derived(config.openrgb?.devices.length ?? 0);
	const live = $derived(app.status?.openrgb ?? null);
	const liveText = $derived.by(() => {
		if (!app.status) return 'The engine isn’t running.';
		if (!live || live.state === 'off') return 'Starting…';
		const n = live.devices.length;
		if (live.state === 'connected') return `Connected to OpenRGB, ${n} ${n === 1 ? 'device' : 'devices'}.`;
		if (live.state === 'waiting') return live.detail ?? 'Waiting for OpenRGB to start.';
		return live.detail ?? 'uncoil couldn’t talk to OpenRGB.';
	});
</script>

<Workspace title={pageTitle('settings')} subtitle={now ? `Your display is ${now}; lighting is at ${Math.round((app.status?.level ?? 1) * 100)}%.` : 'How lighting follows your monitor, and other RGB on your PC.'}>
	<section class="card" aria-labelledby="display-title">
		<h2 id="display-title" class="section-title">When the display sleeps or dims</h2>
		<Toggle label="Turn lighting off when the display sleeps" bind:checked={d.off_when_display_off} hint="Fades the desk out when Windows turns the display off, and back when it wakes, like Synapse does." />
		<Slider label="Brightness while the display is dimmed" min={0} max={100} bind:value={() => Math.round(d.dim_level * 100), (v) => (d.dim_level = v / 100)} format={(v) => `${Math.round(v)}%`} hint="Windows dims the screen shortly before it sleeps. Lighting follows it down to this level." />
		<Slider label="Fade time" min={0} max={5} step={0.1} bind:value={d.fade_s} format={(v) => `${v.toFixed(1)} s`} hint="How long lighting takes to fade out or come back." />
	</section>
	<section class="card" aria-labelledby="rgb-title">
		<div class="head">
			<h2 id="rgb-title" class="section-title">Motherboard, GPU and RAM lighting</h2>
			<p class="hint">uncoil uses OpenRGB for the lighting on the rest of your PC. Razer devices are always left to uncoil.</p>
		</div>
		<OptionList label="OpenRGB" options={MODES} value={mode} onchange={setMode} />
		{#if mode === 'hardware'}
			<p class="hint">
				When you sign in, OpenRGB runs once to put each device listed under <code>openrgb.devices</code> in config.json on its own built-in mode, then exits. Nothing happens until you list a device there ({listed === 0 ? 'none listed now' : `${listed} listed now`}).
			</p>
		{:else if mode === 'live'}
			<p class="hint">OpenRGB runs in the background and uncoil sends it the desk effect, so the rest of your PC lights up with your Razer devices. Brightness and the display settings above apply to it too.</p>
			<p class="status" role="status"><span class="dot" class:on={live?.state === 'connected'} class:warn={live?.state === 'error'} aria-hidden="true"></span>{liveText}</p>
		{/if}
		{#if mode !== 'off'}
			<p class="hint">
				It needs OpenRGB installed and uncoil installed with <code>-OpenRgb</code> (<code>scripts\install-task.ps1 -OpenRgb</code>, one administrator prompt), because OpenRGB needs administrator rights to reach the motherboard and RAM.
			</p>
			<div><button type="button" class="btn-quiet" onclick={() => openExternal(CONFIG_DOC_URL)}>Configuration guide<ExternalLink size={14} /></button></div>
		{/if}
	</section>
</Workspace>

<style>
	.card {
		display: grid;
		gap: 20px;
		max-width: 720px;
		padding: 18px 20px;
		border: var(--hair);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
	}
	.head {
		display: grid;
		gap: 4px;
	}
	.head .section-title {
		margin: 0;
	}
	.hint {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
		max-width: 72ch;
	}
	code {
		white-space: nowrap;
		font-size: 11.5px;
		color: var(--color-ink-2);
	}
	.status {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 0;
		font-weight: 500;
	}
	.dot {
		width: 7px;
		height: 7px;
		flex: none;
		border-radius: 50%;
		background: var(--color-ink-4);
	}
	.dot.on {
		background: var(--color-ok);
	}
	.dot.warn {
		background: var(--color-warn);
	}
	.card :global(.list) {
		margin: -8px -12px;
	}
</style>
