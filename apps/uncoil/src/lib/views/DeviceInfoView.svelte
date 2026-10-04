<script lang="ts">
	import { onMount } from 'svelte';
	import { ExternalLink } from '@lucide/svelte';
	import Workspace from '#lib/components/Workspace.svelte';
	import { daemon, openExternal } from '#lib/api.ts';
	import { app } from '#lib/state.svelte.ts';
	import { pipe, loadDevices, errorText, experimentalBadge, shortName } from '#lib/daemon.svelte.ts';
	import { EXPERIMENTAL, EXPERIMENTAL_TEXT, FEATURE_NAMES, REPORT_URL, STATE_NAMES, checkReads, productId } from '#lib/checks.ts';
	import type { Capabilities, DeskDevice, DeviceKind, FeatureCheck, ProfileInfo } from '#lib/types.ts';

	let { device, desk = null }: { device: { id: string; name: string; kind: DeviceKind }; desk?: DeskDevice | null } = $props();

	onMount(() => {
		if (!pipe.loaded) loadDevices();
	});
	const s = $derived(app.status?.devices.find((d) => d.id === device.id) ?? null);
	const info = $derived(pipe.devices.find((d) => d.id === device.id) ?? null);
	const experimental = $derived(info?.support === 'experimental');

	let profiles = $state<ProfileInfo | null>(null);
	let profileError = $state<string | null>(null);
	$effect(() => {
		if (info?.features.includes('profiles')) {
			daemon<ProfileInfo>('profile.list', device.id)
				.then((p) => (profiles = p))
				.catch((e) => (profileError = errorText(e)));
		}
	});

	// Checks (experimental devices): what has been confirmed on this connection.
	let checks = $state<FeatureCheck[]>([]);
	$effect(() => {
		if (experimental) {
			daemon<Capabilities>('capabilities', device.id)
				.then((c) => (checks = c.checks ?? []))
				.catch(() => (checks = []));
		}
	});
	const shown = $derived(checks.filter((c) => c.state !== 'not_needed'));
	let checking = $state(false);
	let checkError = $state<string | null>(null);
	async function runCheck() {
		checking = true;
		checkError = null;
		try {
			checks = await daemon<FeatureCheck[]>('check.run', device.id);
		} catch (e) {
			checkError = errorText(e);
		} finally {
			checking = false;
		}
	}

	const kind: Record<string, string> = { keyboard: 'Keyboard', mouse: 'Mouse', mousemat: 'Mouse mat', headset: 'Headset', other: 'Device' };
	const statusText = $derived(!app.status ? 'Engine not running' : s ? (s.errors ? `Connected, ${s.errors} error${s.errors === 1 ? '' : 's'}` : 'Connected') : 'Not connected');
	const lit = $derived(!!desk || !!info?.features.includes('lighting'));
	const leds = $derived(!desk ? null : device.kind === 'mousemat' ? 1 : desk.shapes.length);
	const pid = $derived(s?.product_id ?? info?.product_id ?? null);
</script>

<Workspace title={shortName(device.name)} subtitle={kind[device.kind]} badge={experimentalBadge(device.id)}>
	<div class="grid">
		<section class="card" aria-labelledby="conn-title">
			<h2 id="conn-title" class="section-title">Connection</h2>
			<dl>
				<div><dt>Status</dt><dd><span class="dot" class:on={!!s} class:warn={!!s?.errors}></span>{statusText}</dd></div>
				<div><dt>Link</dt><dd>{s?.connection ? s.connection[0].toUpperCase() + s.connection.slice(1) : '—'}</dd></div>
				<div><dt>USB product id</dt><dd class="num">{pid !== null ? productId(pid) : '—'}</dd></div>
				{#if lit}
					<div><dt>Lighting updates</dt><dd class="num">{s ? `${s.fps.toFixed(0)} per second` : '—'}</dd></div>
					<div><dt>LEDs</dt><dd class="num">{leds ?? '—'}</dd></div>
				{:else}
					<div><dt>Lighting</dt><dd>None</dd></div>
				{/if}
				<div><dt>Busy retries</dt><dd class="num">{s ? s.busy_retries : '—'}</dd></div>
			</dl>
		</section>

		<section class="card" aria-labelledby="feat-title">
			<h2 id="feat-title" class="section-title">What uncoil can do with it</h2>
			{#if info}
				<ul class="features">
					{#each info.features as f (f)}<li>{FEATURE_NAMES[f] ?? f}</li>{/each}
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

		{#if experimental}
			<section class="card wide" aria-labelledby="exp-title">
				<div class="exp-head">
					<h2 id="exp-title" class="section-title">{EXPERIMENTAL}</h2>
					<button type="button" class="btn-quiet" onclick={() => openExternal(REPORT_URL)}>Tell us whether it works<ExternalLink size={14} /></button>
				</div>
				<p class="lead">{EXPERIMENTAL_TEXT}</p>
				<p class="note">
					Before uncoil changes anything stored on it, it runs a few read-only checks: it reads current settings and makes sure the answers look right. Until a check passes, that setting can be viewed but not changed. Checks run again each time the device is plugged in.
				</p>
				{#if shown.length}
					<ul class="checks">
						{#each shown as c (c.feature)}
							<li>
								<span class="dot" class:on={c.state === 'passed'} class:warn={c.state === 'failed'} class:hollow={c.state === 'untested'} aria-hidden="true"></span>
								<span class="cname">{FEATURE_NAMES[c.feature] ?? c.feature}</span>
								<span class="reads">Reads {checkReads(c.feature, device.kind)}</span>
								<span class="cstate">{STATE_NAMES[c.state]}</span>
								{#if c.detail}<span class="detail">{c.detail}</span>{/if}
							</li>
						{/each}
					</ul>
				{/if}
				{#if shown.some((c) => c.state === 'failed')}
					<p class="note">
						uncoil couldn't confirm this device answers the way it expects, so it won't change settings stored on it.{lit ? ' Lighting still works.' : ''} Telling us what you see helps fix the device file.
					</p>
				{/if}
				{#if checkError}<p class="outcome bad" role="alert">{checkError}</p>{/if}
				{#if info}
					<div><button type="button" class="btn-quiet" disabled={checking} onclick={runCheck}>{checking ? 'Checking…' : shown.some((c) => c.state !== 'untested') ? 'Run checks again' : 'Run checks'}</button></div>
				{/if}
			</section>
		{/if}
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
	.wide {
		grid-column: 1 / -1;
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
	.dot.hollow {
		background: transparent;
		box-shadow: inset 0 0 0 1px var(--color-ink-4);
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
	.exp-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
	}
	.lead {
		margin: 0;
		color: var(--color-ink);
		max-width: 72ch;
	}
	.note {
		margin: 0;
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.5;
		max-width: 80ch;
	}
	.bad {
		color: var(--color-warn);
	}
	.checks {
		display: grid;
		margin: 0;
		padding: 0;
		list-style: none;
		border-top: var(--hair);
	}
	.checks li {
		display: grid;
		grid-template-columns: 7px 140px minmax(0, 1fr) auto;
		align-items: center;
		gap: 4px 12px;
		padding: 9px 0;
		border-bottom: var(--hair);
	}
	.cname {
		font-weight: 500;
	}
	.reads {
		color: var(--color-ink-3);
		font-size: 12px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.cstate {
		font-size: 12px;
		font-weight: 500;
		color: var(--color-ink-2);
	}
	.detail {
		grid-column: 2 / -1;
		color: var(--color-ink-3);
		font-size: 12px;
	}
	@container view (max-width: 640px) {
		.grid {
			grid-template-columns: minmax(0, 1fr);
		}
		.checks li {
			grid-template-columns: 7px minmax(0, 1fr) auto;
		}
		.reads {
			display: none;
		}
	}
</style>
