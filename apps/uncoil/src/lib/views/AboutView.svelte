<script lang="ts">
	import { inTauri } from '#lib/api.ts';

	const REPO = 'https://github.com/justinthevoid/uncoil';

	// Headline figures: Synapse idling with Chroma lighting, versus uncoild driving the same devices.
	const SYNAPSE_MB = 1400;
	const UNCOIL_MB = 8;

	async function openRepo(e: MouseEvent) {
		if (!inTauri) return; // plain browser: let the link open normally
		e.preventDefault();
		const { openUrl } = await import('@tauri-apps/plugin-opener');
		await openUrl(REPO);
	}
</script>

<section class="view" aria-labelledby="about-title">
	<header>
		<h1 id="about-title">uncoil</h1>
		<p class="lede">
			Razer lighting without Razer Synapse. One small background process drives your keyboard, mouse and
			mouse mat as a single surface.
		</p>
	</header>

	<div class="compare">
		<h2>Memory in use</h2>
		<div class="bars">
			<div class="bar-row">
				<span class="who">Razer Synapse</span>
				<span class="track"><span class="fill synapse" style:width="100%"></span></span>
				<span class="val num">~1.4 GB</span>
			</div>
			<div class="bar-row">
				<span class="who">uncoil engine</span>
				<span class="track">
					<span class="fill uncoil" style:width="{(UNCOIL_MB / SYNAPSE_MB) * 100}%"></span>
				</span>
				<span class="val num">~8 MB</span>
			</div>
		</div>

		<h2>Processes</h2>
		<div class="bars">
			<div class="bar-row">
				<span class="who">Razer Synapse</span>
				<span class="dots" aria-hidden="true">
					{#each Array(17) as _, i (i)}<span class="dot synapse"></span>{/each}
				</span>
				<span class="val num">17</span>
			</div>
			<div class="bar-row">
				<span class="who">uncoil engine</span>
				<span class="dots" aria-hidden="true"><span class="dot uncoil"></span></span>
				<span class="val num">1</span>
			</div>
		</div>
	</div>

	<footer>
		<a href={REPO} target="_blank" rel="noreferrer" onclick={openRepo}>Source code on GitHub</a>
		<span class="meta">Version 0.1.0. Free software under the GPL, version 3 or later.</span>
	</footer>
</section>

<style>
	.view {
		display: grid;
		align-content: start;
		gap: 32px;
		max-width: 640px;
	}
	header {
		display: grid;
		gap: 8px;
	}
	h1 {
		font-family: var(--font-display);
		font-size: 32px;
		font-weight: 600;
		letter-spacing: -0.02em;
		line-height: 1.1;
	}
	.lede {
		color: var(--color-dim);
		font-size: 15px;
		max-width: 52ch;
	}
	.compare {
		display: grid;
		gap: 12px;
		padding: 22px 24px;
		border-radius: 14px;
		background: var(--color-ink-1);
		box-shadow: inset 0 0 0 1px var(--color-line);
	}
	h2 {
		color: var(--color-dim);
		font-size: 13px;
		font-weight: normal;
	}
	h2:not(:first-child) {
		margin-top: 10px;
	}
	.bars {
		display: grid;
		gap: 8px;
	}
	.bar-row {
		display: grid;
		grid-template-columns: 112px 1fr 64px;
		align-items: center;
		gap: 14px;
		font-size: 13px;
	}
	.who {
		color: var(--color-text);
	}
	.val {
		text-align: right;
		color: var(--color-text);
	}
	.track {
		height: 10px;
		border-radius: 3px;
		background: var(--color-well);
		overflow: hidden;
	}
	.fill {
		display: block;
		height: 100%;
		min-width: 3px;
		border-radius: 3px;
	}
	.synapse {
		background: #4a4e55;
	}
	.uncoil {
		background: var(--color-brass);
	}
	.dots {
		display: flex;
		gap: 5px;
	}
	.dot {
		width: 10px;
		height: 10px;
		border-radius: 2px;
	}
	footer {
		display: grid;
		gap: 6px;
		font-size: 13px;
	}
	a {
		color: var(--color-brass);
		text-decoration: underline;
		text-underline-offset: 3px;
		text-decoration-color: var(--color-brass-dim);
		justify-self: start;
	}
	a:hover {
		text-decoration-color: currentColor;
	}
	.meta {
		color: var(--color-faint);
	}
</style>
