<script lang="ts">
	import { onMount } from 'svelte';
	import { flip } from 'svelte/animate';
	import DeskPreview from '#lib/components/DeskPreview.svelte';
	import { getDesk, previewFrame } from '#lib/api.ts';
	import { app } from '#lib/state.svelte.ts';
	import { ms } from '#lib/motion.ts';
	import type { Config, DeskDevice } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	let desk = $state<DeskDevice[]>([]);
	let colors = $state<string[][]>([]);
	onMount(() => {
		let alive = true;
		const t0 = performance.now();
		getDesk($state.snapshot(config) as Config).then((d) => alive && (desk = d));
		const id = setInterval(async () => {
			if (!desk.length || (document.hidden && colors.length)) return;
			const f = await previewFrame($state.snapshot(config) as Config, 6 + (performance.now() - t0) / 1000);
			if (alive) colors = f;
		}, 80);
		return () => {
			alive = false;
			clearInterval(id);
		};
	});

	const kindLabel: Record<string, string> = { keyboard: 'Keyboard', mouse: 'Mouse', mousemat: 'Mouse mat', headset: 'Headset', other: 'Device' };

	// Every known device, joined with what the engine reports. Connected first.
	const rows = $derived.by(() => {
		const status = app.status;
		return desk
			.map((d) => {
				const s = status?.devices.find((x) => x.id === d.id);
				return {
					id: d.id,
					name: d.name.replace(/^Razer /, ''),
					kind: kindLabel[d.kind] ?? 'Device',
					leds: d.kind === 'mousemat' ? 1 : d.shapes.length,
					live: !!s,
					connection: s?.connection ? s.connection[0].toUpperCase() + s.connection.slice(1) : '—',
					fps: s ? s.fps.toFixed(0) : '—',
					errors: s ? s.errors : 0
				};
			})
			.sort((a, b) => Number(b.live) - Number(a.live));
	});
	const away = $derived(new Set(app.status ? rows.filter((r) => !r.live).map((r) => r.id) : []));
</script>

<section class="devices" aria-labelledby="devices-title">
	<header class="head">
		<h1 id="devices-title" class="page-title">Devices</h1>
		<p class="lede">
			{app.status ? `${app.status.devices.length} of ${desk.length} connected.` : 'The engine isn’t running, so this shows your desk as configured.'}
			Unplugged devices are picked up again within a few seconds of coming back.
		</p>
	</header>

	<div class="card">
		<table>
			<thead>
				<tr>
					<th scope="col">Device</th>
					<th scope="col">Status</th>
					<th scope="col">Connection</th>
					<th scope="col" class="r">LEDs</th>
					<th scope="col" class="r">Updates / s</th>
				</tr>
			</thead>
			<tbody>
				{#each rows as r (r.id)}
					<tr animate:flip={{ duration: ms(420) }} class:away={!r.live && !!app.status}>
						<td><span class="dname">{r.name}</span><span class="dkind">{r.kind}</span></td>
						<td>
							<span class="status" class:live={r.live} class:warn={r.errors > 0}>
								<span class="lamp" aria-hidden="true"></span>
								{!app.status ? 'Engine off' : !r.live ? 'Not connected' : r.errors > 0 ? `${r.errors} error${r.errors === 1 ? '' : 's'}` : 'Connected'}
							</span>
						</td>
						<td>{r.connection}</td>
						<td class="num r">{r.leds}</td>
						<td class="num r">{r.fps}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

	<div class="stage">
		<DeskPreview {desk} {colors} {away} />
	</div>
</section>

<style>
	.devices {
		display: grid;
		grid-template-rows: auto auto minmax(200px, 1fr);
		gap: 16px;
		height: 100%;
		min-height: 0;
	}
	.lede {
		margin: 6px 0 0;
		color: var(--color-ink-3);
		font-size: 13px;
		max-width: 75ch;
	}
	.card {
		padding: 6px 20px;
		border-radius: var(--radius-lg);
		background: var(--color-raised);
		border: 1px solid var(--color-seam);
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
		padding: 12px 12px 10px 0;
		border-bottom: 1px solid var(--color-seam-2);
	}
	td {
		padding: 12px 12px 12px 0;
		border-bottom: 1px solid var(--color-seam);
		font-size: 13px;
		vertical-align: middle;
	}
	tbody tr:last-child td {
		border-bottom: 0;
	}
	.r {
		text-align: right;
	}
	.dname {
		display: block;
		font-size: 14px;
		font-weight: 500;
	}
	.dkind {
		color: var(--color-ink-3);
		font-size: 12px;
	}
	.status {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		color: var(--color-ink-3);
	}
	.lamp {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-ink-4);
	}
	.status.live {
		color: var(--color-ink);
	}
	.status.live .lamp {
		background: #3fb950;
	}
	.status.warn .lamp {
		background: var(--color-fac-yellow);
	}
	tr {
		transition: opacity var(--t-slow) var(--ease);
	}
	tr.away {
		opacity: 0.55;
	}
	.stage {
		min-height: 0;
		padding: 18px;
		border-radius: var(--radius-lg);
		background: radial-gradient(ellipse at 50% 40%, #151515, #0d0d0d 70%);
		border: 1px solid var(--color-seam);
	}
	@container view (max-width: 820px) {
		.devices {
			grid-template-rows: auto auto 240px;
			height: auto;
		}
	}
</style>
