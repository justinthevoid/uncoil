<script lang="ts">
	// Settings for one effect (top-level or a Studio layer). Edits the effect in place.
	import Dial from './Dial.svelte';
	import Slider from './Slider.svelte';
	import Toggle from './Toggle.svelte';
	import GelPicker from './GelPicker.svelte';
	import Segmented from './Segmented.svelte';
	import type { LayerEffect, Rgb } from '#lib/types.ts';

	let { effect }: { effect: LayerEffect } = $props();

	// Speed on a log scale: reads evenly from a slow drift (60 s per cycle) to a quick cycle (2 s).
	const SLOW = 60;
	const FAST = 2;
	const toPos = (period: number) => (100 * Math.log(period / SLOW)) / Math.log(FAST / SLOW);
	const toPeriod = (pos: number) => Math.round(SLOW * Math.pow(FAST / SLOW, pos / 100) * 10) / 10;
	const fmtPeriod = (p: number) => `${p < 10 ? p.toFixed(1) : Math.round(p)} s`;
	const pct = (v: number) => `${Math.round(v)}%`;
	const secs = (v: number) => `${v.toFixed(1)} s`;

	// Breathing: 0 colours = rainbow, 1 = single, 2 = alternate.
	const breathMode = $derived(effect.kind === 'breathing' ? (effect.colors.length === 0 ? 'rainbow' : effect.colors.length === 1 ? 'one' : 'two') : 'one');
	function setBreathMode(m: 'rainbow' | 'one' | 'two') {
		if (effect.kind !== 'breathing') return;
		const a: Rgb = effect.colors[0] ?? [224, 163, 62];
		const b: Rgb = effect.colors[1] ?? [77, 127, 184];
		effect.colors = m === 'rainbow' ? [] : m === 'one' ? [a] : [a, b];
	}
	const starMode = $derived(effect.kind === 'starlight' && effect.colors.length ? 'chosen' : 'random');
</script>

<div class="settings">
	{#if effect.kind === 'wave'}
		{@const e = effect}
		<Dial label="Direction" bind:value={e.angle_deg} />
		<Slider label="Speed" min={0} max={100} step={0.5} bind:value={() => toPos(e.period_s), (v) => (e.period_s = toPeriod(v))} format={(v) => `${fmtPeriod(toPeriod(v))} per cycle`} ends={['Slower', 'Faster']} />
		<Slider label="Band width" min={6} max={60} bind:value={e.wavelength} format={(v) => `${v} keys`} ends={['Tight', 'Broad']} />
		<Toggle label="Reverse direction" bind:checked={e.reverse} />
	{:else if effect.kind === 'spectrum'}
		{@const e = effect}
		<Slider label="Speed" min={0} max={100} step={0.5} bind:value={() => toPos(e.period_s), (v) => (e.period_s = toPeriod(v))} format={(v) => `${fmtPeriod(toPeriod(v))} per cycle`} ends={['Slower', 'Faster']} />
	{:else if effect.kind === 'static'}
		{@const e = effect}
		<GelPicker label="Colour" value={e.color} onchange={(c) => c && (e.color = c)} />
	{:else if effect.kind === 'breathing'}
		{@const e = effect}
		<Segmented
			label="Colours"
			options={[
				{ value: 'rainbow' as const, label: 'Rainbow' },
				{ value: 'one' as const, label: 'One colour' },
				{ value: 'two' as const, label: 'Two colours' }
			]}
			value={breathMode}
			onchange={setBreathMode}
		/>
		{#if e.colors.length >= 1}
			<GelPicker label={e.colors.length === 2 ? 'First colour' : 'Colour'} value={e.colors[0]} onchange={(c) => c && (e.colors[0] = c)} />
		{/if}
		{#if e.colors.length === 2}
			<GelPicker label="Second colour" value={e.colors[1]} onchange={(c) => c && (e.colors[1] = c)} />
		{/if}
		<Slider label="Speed" min={0} max={100} step={0.5} bind:value={() => toPos(e.period_s), (v) => (e.period_s = toPeriod(v))} format={(v) => `${fmtPeriod(toPeriod(v))} per breath`} ends={['Slower', 'Faster']} />
	{:else if effect.kind === 'starlight'}
		{@const e = effect}
		<Segmented
			label="Colours"
			options={[
				{ value: 'random' as const, label: 'Random' },
				{ value: 'chosen' as const, label: 'One colour' }
			]}
			value={starMode}
			onchange={(m) => (e.colors = m === 'random' ? [] : [e.colors[0] ?? [255, 244, 192]])}
		/>
		{#if e.colors.length}
			<GelPicker label="Colour" value={e.colors[0]} onchange={(c) => c && (e.colors = [c])} />
		{/if}
		<Slider label="How many" min={2} max={60} bind:value={() => Math.round(e.density * 100), (v) => (e.density = v / 100)} format={pct} ends={['Sparse', 'Busy']} />
		<Slider label="Twinkle length" min={0.3} max={5} step={0.1} bind:value={e.twinkle_s} format={secs} ends={['Quick', 'Slow']} />
	{:else if effect.kind === 'fire'}
		{@const e = effect}
		<Slider label="Flame height" min={10} max={100} bind:value={() => Math.round(e.height * 100), (v) => (e.height = v / 100)} format={pct} ends={['Low', 'Tall']} />
		<Slider label="Speed" min={25} max={300} bind:value={() => Math.round(e.speed * 100), (v) => (e.speed = v / 100)} format={(v) => `${(v / 100).toFixed(2)}×`} ends={['Calm', 'Wild']} />
	{:else if effect.kind === 'wheel'}
		{@const e = effect}
		<Slider label="Speed" min={0} max={100} step={0.5} bind:value={() => toPos(e.period_s), (v) => (e.period_s = toPeriod(v))} format={(v) => `${fmtPeriod(toPeriod(v))} per turn`} ends={['Slower', 'Faster']} />
		<Toggle label="Turn the other way" bind:checked={e.reverse} />
	{:else if effect.kind === 'reactive'}
		{@const e = effect}
		<GelPicker label="Colour" rainbow value={e.color} onchange={(c) => (e.color = c)} />
		<Slider label="Fade" min={0.2} max={4} step={0.1} bind:value={e.fade_s} format={secs} ends={['Quick', 'Slow']} />
		<p class="note">Lights the keys you press. While this effect is on, the engine listens for key presses and keeps only where on the keyboard each one was, never what you typed.</p>
	{:else if effect.kind === 'ripple'}
		{@const e = effect}
		<GelPicker label="Colour" rainbow value={e.color} onchange={(c) => (e.color = c)} />
		<Slider label="Speed" min={4} max={40} bind:value={e.speed} format={(v) => `${Math.round(v)} keys/s`} ends={['Slow', 'Fast']} />
		<Slider label="Ring width" min={0.5} max={6} step={0.5} bind:value={e.width} format={(v) => `${v} keys`} ends={['Thin', 'Wide']} />
		<Slider label="Fade" min={0.3} max={4} step={0.1} bind:value={e.fade_s} format={secs} ends={['Quick', 'Slow']} />
		<p class="note">Spreads from the keys you press. While this effect is on, the engine listens for key presses and keeps only where on the keyboard each one was, never what you typed.</p>
	{:else if effect.kind === 'audio_meter'}
		{@const e = effect}
		<Slider label="Sensitivity" min={25} max={400} bind:value={() => Math.round(e.sensitivity * 100), (v) => (e.sensitivity = v / 100)} format={(v) => `${(v / 100).toFixed(2)}×`} ends={['Less', 'More']} />
		<p class="note">Follows the volume level of whatever your PC is playing. Only the level is read, never the sound itself.</p>
	{:else}
		<p class="note">Lights stay dark.</p>
	{/if}
</div>

<style>
	.settings {
		display: grid;
		gap: 18px;
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
	}
</style>
