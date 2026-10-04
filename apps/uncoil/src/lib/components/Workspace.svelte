<script lang="ts">
	// The screen frame every feature uses: a title row with its tools, the stage where the device or desk
	// is drawn large, and an optional details panel on the right for the selected thing.
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		subtitle?: string;
		tools?: Snippet;
		children: Snippet;
		panel?: Snippet;
		panelLabel?: string;
		/** A quiet tag after the title, e.g. "Experimental", with a one-line explanation as its tooltip. */
		badge?: { text: string; title: string } | null;
	}
	let { title, subtitle, tools, children, panel, panelLabel = 'Details', badge = null }: Props = $props();
</script>

<div class="ws" class:has-panel={!!panel}>
	<section class="stage">
		<header class="head">
			<div class="titles">
				<div class="title-row">
					<h1 class="page-title">{title}</h1>
					{#if badge}<span class="badge" title={badge.title}>{badge.text}</span>{/if}
				</div>
				{#if subtitle}<p class="sub">{subtitle}</p>{/if}
			</div>
			{#if tools}<div class="tools">{@render tools()}</div>{/if}
		</header>
		<div class="body">{@render children()}</div>
	</section>
	{#if panel}
		<aside class="panel" aria-label={panelLabel}>{@render panel()}</aside>
	{/if}
</div>

<style>
	.ws {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		height: 100%;
		min-height: 0;
	}
	.ws.has-panel {
		grid-template-columns: minmax(0, 1fr) clamp(290px, 28%, 340px);
	}
	.stage {
		display: grid;
		grid-template-rows: auto 1fr;
		gap: 18px;
		min-width: 0;
		min-height: 0;
		padding: 18px 28px 22px;
		overflow: auto;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 12px;
		min-height: 34px;
	}
	.titles {
		margin-right: auto;
		min-width: 0;
	}
	.title-row {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.badge {
		padding: 1px 8px;
		border: var(--hair-strong);
		border-radius: var(--radius-sm);
		color: var(--color-ink-2);
		font-size: 11.5px;
		font-weight: 500;
		line-height: 18px;
		white-space: nowrap;
	}
	.sub {
		margin: 2px 0 0;
		color: var(--color-ink-3);
		font-size: 12px;
	}
	.tools {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.body {
		min-height: 0;
		display: grid;
		align-content: start;
		gap: 22px;
	}
	.panel {
		min-height: 0;
		padding: 18px;
		border-left: var(--hair);
		background: var(--color-raised);
		overflow: auto;
	}
	@container view (max-width: 720px) {
		.ws.has-panel {
			grid-template-columns: minmax(0, 1fr);
			height: auto;
		}
		.panel {
			border-left: 0;
			border-top: var(--hair);
		}
		.stage {
			overflow: visible;
		}
	}
</style>
