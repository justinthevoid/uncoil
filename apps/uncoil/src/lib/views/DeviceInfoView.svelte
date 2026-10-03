<script lang="ts">
	import { onMount } from 'svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import { daemon } from '#lib/api.ts';
	import { app } from '#lib/state.svelte.ts';
	import { pipe, loadDevices, errorText } from '#lib/daemon.svelte.ts';
	import type { DeskDevice, ProfileInfo } from '#lib/types.ts';

	let { device }: { device: DeskDevice } = $props();

	onMount(() => {
		if (!pipe.loaded) loadDevices();
	});
	const s = $derived(app.status?.devices.find((d) => d.id === device.id) ?? null);
	const info = $derived(pipe.devices.find((d) => d.id === device.id) ?? null);

	let profiles = $state<ProfileInfo | null>(null);
	let profileError = $state<string | null>(null);
	$effect(() => {
		if (info?.features.includes('profiles')) {
			daemon<ProfileInfo>('profile.list', device.id)
				.then((p) => (profiles = p))
				.catch((e) => (profileError = errorText(e)));
		}
	});

	const kind: Record<string, string> = { keyboard: 'Keyboard', mouse: 'Mouse', mousemat: 'Mouse mat', headset: 'Headset', other: 'Device' };
	const statusText = $derived(!app.status ? 'Engine not running' : s ? (s.errors ? `Connected, ${s.errors} error${s.errors === 1 ? '' : 's'}` : 'Connected') : 'Not connected');
	const leds = $derived(device.kind === 'mousemat' ? 1 : device.shapes.length);
	const hex = (n: number) => '0x' + n.toString(16).toUpperCase().padStart(4, '0');
	const FEATURE: Record<string, string> = { lighting: 'Lighting', hw_effects: 'Onboard effects', keymap: 'Key remapping', profiles: 'Onboard profiles', dial: 'Command dial', oled: 'Screen' };
</script>

<Workspace title={device.name.replace(/^Razer /, '')} subtitle={kind[device.kind]}>
	<div class="grid">
		<section class="card" aria-labelledby="conn-title">
			<h2 id="conn-title" class="section-title">Connection</h2>
			<dl>
				<div><dt>Status</dt><dd><span class="dot" class:on={!!s} class:warn={!!s?.errors}></span>{statusText}</dd></div>
				<div><dt>Link</dt><dd>{s?.connection ? s.connection[0].toUpperCase() + s.connection.slice(1) : '—'}</dd></div>
				<div><dt>USB product id</dt><dd class="num">{s ? hex(s.product_id) : '—'}</dd></div>
				<div><dt>Lighting updates</dt><dd class="num">{s ? `${s.fps.toFixed(0)} per second` : '—'}</dd></div>
				<div><dt>LEDs</dt><dd class="num">{leds}</dd></div>
				<div><dt>Busy retries</dt><dd class="num">{s ? s.busy_retries : '—'}</dd></div>
			</dl>
		</section>

		<section class="card" aria-labelledby="feat-title">
			<h2 id="feat-title" class="section-title">What uncoil can do with it</h2>
			{#if info}
				<ul class="features">
					{#each info.features as f (f)}<li>{FEATURE[f] ?? f}</li>{/each}
				</ul>
			{:else}
				<p class="note">{pipe.unreachable ? 'The engine isn’t answering, so this can’t be checked right now.' : 'Plug the device in to see what it supports.'}</p>
			{/if}
			{#if profiles}
				<p class="note">Onboard profiles: {profiles.count} of {profiles.max} slots in use{profiles.active ? `, profile ${profiles.active} active` : ''}. Switching profiles from uncoil isn't possible yet.</p>
			{:else if profileError}
				<p class="note bad">{profileError}</p>
			{/if}
		</section>
	</div>
</Workspace>

<style>
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
	dl {
		display: grid;
		gap: 0;
		margin: 0;
	}
	dl div {
		display: flex;
		justify-content: space-between;
		gap: 16px;
		padding: 9px 0;
		border-bottom: var(--hair);
	}
	dl div:last-child {
		border-bottom: 0;
	}
	dt {
		color: var(--color-ink-3);
	}
	dd {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 0;
		font-weight: 500;
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--color-ink-4);
	}
	.dot.on {
		background: var(--color-ok);
	}
	.dot.warn {
		background: var(--color-warn);
	}
	.features {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.features li {
		padding: 4px 10px;
		border-radius: 999px;
		background: var(--color-surface-2);
		font-size: 12px;
		font-weight: 500;
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
	}
	.bad {
		color: var(--color-warn);
	}
	@container view (max-width: 640px) {
		.grid {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
