<script lang="ts">
	import { app } from '#lib/state.svelte.ts';

	const now = () => Math.floor(Date.now() / 1000);

	function uptime(started: number) {
		const s = Math.max(0, now() - started);
		const h = Math.floor(s / 3600);
		const m = Math.floor((s % 3600) / 60);
		if (h > 0) return `${h} h ${m} min`;
		if (m > 0) return `${m} min`;
		return 'less than a minute';
	}

	const displayWords: Record<string, string> = {
		on: 'Display on',
		dimmed: 'Display dimmed',
		off: 'Display asleep'
	};

	const hexId = (n: number) => n.toString(16).toUpperCase().padStart(4, '0');
	const capital = (s: string) => (s ? s[0].toUpperCase() + s.slice(1) : 'Unknown');
</script>

<section class="view" aria-labelledby="devices-title">
	<header>
		<h1 id="devices-title">Devices</h1>
		{#if app.status}
			<p class="sub">
				Engine running for {uptime(app.status.started_unix)}.
				{displayWords[app.status.display] ?? `Display ${app.status.display}`}, lighting at
				{Math.round(app.status.level * 100)}%.
			</p>
		{/if}
	</header>

	{#if !app.statusKnown}
		<p class="sub">Checking the engine…</p>
	{:else if !app.status}
		<div class="empty">
			<svg viewBox="0 0 40 40" width="40" height="40" aria-hidden="true">
				<circle cx="20" cy="20" r="17" />
				<path d="M13 20h14" />
			</svg>
			<h2>The lighting engine isn't running</h2>
			<p>
				The engine, uncoild, is the small background process that talks to your devices. Start it and
				each connected device appears here with its frame rate and error counts.
			</p>
			<p>Your lighting settings are saved either way and apply as soon as the engine starts.</p>
		</div>
	{:else if app.status.devices.length === 0}
		<div class="empty">
			<h2>No supported devices found</h2>
			<p>
				The engine is running but hasn't found a device it knows. Check the cable or wireless dongle; the
				list updates on its own.
			</p>
		</div>
	{:else}
		<table>
			<thead>
				<tr>
					<th scope="col">Device</th>
					<th scope="col">Connection</th>
					<th scope="col" class="n">Frame rate</th>
					<th scope="col" class="n">Busy retries</th>
					<th scope="col" class="n">Errors</th>
				</tr>
			</thead>
			<tbody>
				{#each app.status.devices as d (d.id + d.product_id)}
					<tr>
						<th scope="row">
							<span class="name">{d.name}</span>
							<span class="pid num">USB ID {hexId(d.product_id)}</span>
						</th>
						<td>{capital(d.connection)}</td>
						<td class="n num">
							{#if d.fps > 0}{d.fps.toFixed(1)} fps{:else}<span class="faint">Idle</span>{/if}
						</td>
						<td class="n num">{d.busy_retries.toLocaleString()}</td>
						<td class="n num" class:warn={d.errors > 0}>{d.errors.toLocaleString()}</td>
					</tr>
				{/each}
			</tbody>
		</table>
		<p class="foot">
			Busy retries are normal: a device sometimes asks the engine to wait a moment. Errors are frames that
			failed to send.
		</p>
	{/if}
</section>

<style>
	.view {
		display: grid;
		align-content: start;
		gap: 24px;
		max-width: 820px;
	}
	header {
		display: grid;
		gap: 6px;
	}
	h1 {
		font-family: var(--font-display);
		font-size: 24px;
		font-weight: 600;
		letter-spacing: -0.01em;
	}
	.sub {
		color: var(--color-dim);
		font-size: 13px;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 13px;
	}
	th,
	td {
		padding: 12px 14px;
		text-align: left;
		font-weight: normal;
		border-bottom: 1px solid var(--color-line);
	}
	thead th {
		color: var(--color-faint);
		font-size: 12px;
		padding-top: 0;
	}
	.name {
		display: block;
		color: var(--color-text);
	}
	.pid {
		display: block;
		margin-top: 2px;
		color: var(--color-faint);
		font-size: 12px;
	}
	td {
		color: var(--color-dim);
	}
	.n {
		text-align: right;
	}
	.warn {
		color: var(--color-warn);
	}
	.faint {
		color: var(--color-faint);
	}
	.foot {
		color: var(--color-faint);
		font-size: 12px;
		max-width: 64ch;
	}
	.empty {
		display: grid;
		gap: 10px;
		justify-items: start;
		max-width: 56ch;
		padding: 28px;
		border-radius: 14px;
		background: var(--color-ink-1);
		box-shadow: inset 0 0 0 1px var(--color-line);
	}
	.empty svg {
		fill: none;
		stroke: var(--color-faint);
		stroke-width: 1.5;
		stroke-linecap: round;
		margin-bottom: 4px;
	}
	.empty h2 {
		font-size: 16px;
		font-weight: 600;
	}
	.empty p {
		color: var(--color-dim);
		font-size: 13px;
	}
</style>
