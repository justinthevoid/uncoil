<script lang="ts">
	// Shown above an editor on an experimental device until the read-only check for its settings passes.
	// "Run check" reads a few current values (never writes) and the editor unlocks when they look right.
	import { fade } from 'svelte/transition';
	import { ShieldCheck } from '@lucide/svelte';
	import { daemon } from '#lib/api.ts';
	import { errorText } from '#lib/daemon.svelte.ts';
	import { checkOf, checkReads, joinWords, locked } from '#lib/checks.ts';
	import { ms } from '#lib/motion.ts';
	import type { Capabilities, Feature, FeatureCheck } from '#lib/types.ts';

	interface Props {
		deviceId: string;
		caps: Capabilities | null;
		/** The features this editor writes. */
		features: Feature[];
		/** Called with the fresh results after a check run. */
		onchecks: (checks: FeatureCheck[]) => void;
		compact?: boolean;
	}
	let { deviceId, caps, features, onchecks, compact = false }: Props = $props();

	const pending = $derived(features.filter((f) => locked(caps, f)));
	const failed = $derived(pending.map((f) => checkOf(caps, f)!).filter((c) => c.state === 'failed'));
	const kind = $derived(caps?.kind ?? 'other');
	const word = $derived(kind === 'keyboard' ? 'keyboard' : kind === 'mouse' ? 'mouse' : 'device');
	const reads = $derived(joinWords([...new Set(pending.map((f) => checkReads(f, kind)))]));
	const lit = $derived(!!caps?.features.includes('lighting'));

	let busy = $state(false);
	let error = $state<string | null>(null);

	async function run() {
		busy = true;
		error = null;
		try {
			onchecks(await daemon<FeatureCheck[]>('check.run', deviceId));
		} catch (e) {
			error = errorText(e);
		} finally {
			busy = false;
		}
	}
</script>

{#if pending.length}
	<div class="notice" class:compact role="region" aria-label="Check before changing settings" out:fade={{ duration: ms(140) }}>
		<span class="icon" aria-hidden="true"><ShieldCheck size={18} strokeWidth={1.75} /></span>
		<div class="text">
			{#if failed.length}
				<p class="title">Changes are switched off</p>
				<p>uncoil couldn't confirm this {word} answers the way it expects, so it won't change settings stored on it.{lit ? ' Lighting still works.' : ''}</p>
				{#each failed as c (c.feature)}
					{#if c.detail}<p class="outcome bad">{c.detail}</p>{/if}
				{/each}
			{:else}
				<p class="title">Check before changing settings</p>
				<p>This {word} is experimental. Before uncoil changes anything stored on it, it reads {reads} to make sure the {word} answers the way it expects. Nothing is written.</p>
			{/if}
			{#if error}<p class="outcome bad" role="alert">{error}</p>{/if}
			<div class="actions">
				<button type="button" class="btn-quiet" disabled={busy} onclick={run}>{busy ? 'Checking…' : failed.length ? 'Run check again' : 'Run check'}</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.notice {
		display: grid;
		grid-template-columns: 18px minmax(0, 1fr);
		gap: 12px;
		padding: 14px 16px;
		border: var(--hair-strong);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		max-width: 980px;
	}
	.notice.compact {
		grid-template-columns: minmax(0, 1fr);
		padding: 12px 14px;
	}
	.compact .icon {
		display: none;
	}
	.icon {
		color: var(--color-ink-3);
		padding-top: 1px;
	}
	.text {
		display: grid;
		gap: 6px;
	}
	p {
		margin: 0;
		color: var(--color-ink-2);
		font-size: 12.5px;
		line-height: 1.5;
		max-width: 64ch;
	}
	.title {
		color: var(--color-ink);
		font-size: 13px;
		font-weight: 600;
	}
	.actions {
		margin-top: 4px;
	}
</style>
