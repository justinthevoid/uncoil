<script lang="ts">
	import { onMount } from 'svelte';
	import { flip } from 'svelte/animate';
	import { fade } from 'svelte/transition';
	import { ms } from '#lib/motion.ts';
	import CodeStrip from '#lib/components/CodeStrip.svelte';
	import { getDesk, previewFrame } from '#lib/api.ts';
	import { app } from '#lib/state.svelte.ts';
	import type { Config, DeskDevice } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	let desk = $state<DeskDevice[]>([]);
	let colors = $state<string[][]>([]);
	onMount(() => {
		let alive = true;
		const t0 = performance.now();
		getDesk($state.snapshot(config) as Config).then((d) => alive && (desk = d));
		const id = setInterval(async () => {
			if (!desk.length) return;
			const f = await previewFrame($state.snapshot(config) as Config, (performance.now() - t0) / 1000);
			if (alive) colors = f;
		}, 160);
		return () => {
			alive = false;
			clearInterval(id);
		};
	});

	const kindLabel: Record<string, string> = { keyboard: 'Keyboard', mouse: 'Mouse', mousemat: 'Mouse mat', headset: 'Headset', other: 'Device' };

	// Every known device on the desk, joined with what the engine reports. Live first, then away.
	const rows = $derived.by(() => {
		const status = app.status;
		return desk
			.map((d, i) => {
				const s = status?.devices.find((x) => x.id === d.id);
				return {
					id: d.id,
					index: i,
					code: `2${String(i + 1).padStart(2, '0')}`,
					name: d.name,
					kind: kindLabel[d.kind] ?? 'Device',
					leds: d.kind === 'mousemat' ? 1 : d.shapes.length,
					live: !!s,
					pid: s ? `0x${s.product_id.toString(16).toUpperCase().padStart(4, '0')}` : '—',
					connection: s?.connection || '—',
					fps: s ? s.fps.toFixed(1) : '—',
					retries: s ? String(s.busy_retries) : '—',
					errors: s ? s.errors : 0
				};
			})
			.sort((a, b) => Number(b.live) - Number(a.live));
	});

	function strip(i: number) {
		const c = colors[i];
		if (!c?.length) return [];
		const n = Math.min(16, c.length);
		return Array.from({ length: n }, (_, k) => c[Math.floor(((k + 0.5) / n) * c.length)]);
	}

	// Desk diagram: device outlines to scale.
	const view = $derived.by(() => {
		if (!desk.length) return null;
		const xs = desk.flatMap((d) => [d.x, d.x + d.w]);
		const ys = desk.flatMap((d) => [d.y, d.y + d.h]);
		const x0 = Math.min(...xs) - 0.6;
		const y0 = Math.min(...ys) - 0.6;
		return { x0, y0, w: Math.max(...xs) - x0 + 0.6, h: Math.max(...ys) - y0 + 0.6 };
	});
</script>

<section class="devices" aria-labelledby="devices-title">
	<header class="head">
		<h1 id="devices-title" class="title"><span class="display fac">FAC 200</span><span class="caps name">Devices</span></h1>
		<p class="lede">{app.status ? `${app.status.devices.length} of ${desk.length} devices live` : 'Engine not running: showing the desk as configured'}</p>
	</header>

	<div class="body">
		<div class="table-wrap">
			<table>
				<thead>
					<tr class="caps-sm">
						<th scope="col">No.</th>
						<th scope="col">Device</th>
						<th scope="col">Live colour</th>
						<th scope="col" class="r opt">LEDs</th>
						<th scope="col">Link</th>
						<th scope="col" class="opt">PID</th>
						<th scope="col" class="r">FPS</th>
						<th scope="col" class="r">Retries</th>
						<th scope="col">State</th>
					</tr>
				</thead>
				<tbody>
					{#each rows as r (r.id)}
						<tr animate:flip={{ duration: ms(420) }} class:away={!r.live && !!app.status}>
							<td class="num code">{r.code}</td>
							<td><span class="dname">{r.name.replace(/^Razer /, '')}</span><span class="dkind caps-sm">{r.kind}</span></td>
							<td class="strip-cell"><CodeStrip colors={r.live || !app.status ? strip(r.index) : []} label="{r.name} live colours" blocks={16} /></td>
							<td class="num r opt">{r.leds}</td>
							<td class="caps-sm">{r.connection}</td>
							<td class="num opt">{r.pid}</td>
							<td class="num r">{r.fps}</td>
							<td class="num r">{r.retries}</td>
							<td>
								{#key `${r.live}-${r.errors}`}
									<span class="state caps-sm" class:live={r.live} class:warn={r.errors > 0} in:fade={{ duration: ms(260) }}>
										{!app.status ? 'Idle' : !r.live ? 'Away' : r.errors > 0 ? `${r.errors} error${r.errors === 1 ? '' : 's'}` : 'Live'}
									</span>
								{/key}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
			<p class="foot">Devices listed here are the ones uncoil knows how to drive. A device stays listed as <em>Away</em> when it is unplugged or asleep; it is picked up again within five seconds of returning.</p>
		</div>

		{#if view}
			<figure class="desk">
				<svg viewBox="{view.x0} {view.y0} {view.w} {view.h}" role="img" aria-label="Desk layout to scale">
					{#each desk as d, i (d.id)}
						{@const live = !app.status || app.status.devices.some((s) => s.id === d.id)}
						{#if d.kind === 'mouse'}
							<ellipse cx={d.x + d.w / 2} cy={d.y + d.h / 2} rx={d.w / 2} ry={d.h / 2} class="frame" class:dim={!live} fill="none" />
						{:else}
							<rect x={d.x} y={d.y} width={d.w} height={d.h} class="frame" class:dim={!live} fill="none" />
						{/if}
						{#if d.kind !== 'mousemat'}
							{#each d.shapes as s, k (k)}
								{#if s.is_key}
									<rect x={s.x - s.w / 2 + 0.08} y={s.y - s.h / 2 + 0.08} width={s.w - 0.16} height={s.h - 0.16} class="key" style:fill={live ? colors[i]?.[k] : undefined} />
								{:else}
									<rect x={s.x - 0.14} y={s.y - 0.14} width="0.28" height="0.28" class="led" style:fill={live ? colors[i]?.[k] : undefined} />
								{/if}
							{/each}
						{/if}
					{/each}
				</svg>
				<figcaption class="caps-sm">Desk to scale · 1 unit = 19.05 mm</figcaption>
			</figure>
		{/if}
	</div>
</section>

<style>
	.devices {
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
	}
	.name {
		color: var(--color-ink-2);
		letter-spacing: 0.32em;
	}
	.lede {
		margin: 0;
		color: var(--color-ink-2);
		font-size: 13px;
	}
	.body {
		display: grid;
		grid-template-rows: auto 1fr;
		min-height: 0;
		overflow: auto;
	}
	.table-wrap {
		padding: 8px 24px 18px;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th {
		text-align: left;
		color: var(--color-ink-3);
		font-weight: 500;
		padding: 12px 10px 10px 0;
		border-bottom: var(--hair-strong);
	}
	td {
		padding: 14px 10px 14px 0;
		border-bottom: var(--hair);
		vertical-align: middle;
		font-size: 13px;
	}
	.r {
		text-align: right;
	}
	.code {
		color: var(--color-ink-3);
		font-stretch: 112%;
		letter-spacing: 0.06em;
	}
	.dname {
		display: block;
		font-size: 14px;
	}
	.dkind {
		color: var(--color-ink-3);
	}
	.strip-cell {
		width: 180px;
	}
	.state {
		color: var(--color-ink-3);
	}
	.state.live {
		color: var(--color-fac-red);
	}
	.state.warn {
		color: var(--color-fac-yellow);
	}
	tr {
		transition: opacity var(--t-slow) var(--ease);
	}
	tr.away {
		opacity: 0.55;
	}
	.foot {
		margin: 14px 0 0;
		color: var(--color-ink-3);
		font-size: 12px;
		max-width: 70ch;
	}
	.foot em {
		font-style: normal;
		color: var(--color-ink-2);
	}
	.desk {
		margin: 0;
		padding: 8px 24px 18px;
		display: grid;
		gap: 10px;
		min-height: 220px;
	}
	.desk svg {
		width: 100%;
		height: 100%;
		max-height: 300px;
	}
	.frame {
		fill: none;
		stroke: var(--color-seam-2);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
	}
	.frame.dim {
		stroke-dasharray: 4 3;
	}
	.key,
	.led {
		fill: var(--color-seam);
		transition: fill 140ms linear;
	}
	figcaption {
		color: var(--color-ink-4);
	}

	@container view (max-width: 820px) {
		.opt {
			display: none;
		}
		.strip-cell {
			width: 128px;
		}
	}
</style>
