// Per-device lighting on the Lighting page, as a simple Studio: a whole-desk base layer, then zones on top,
// each covering some devices or some picked lights, fully opaque. A desk with no zones is a plain effect.
// These helpers edit a config's effect in place (it is Svelte state), the way the Studio page does.
import { lightsOf } from './masks';
import type { Config, LayerEffect, Mask, StudioLayer } from './types';

/** What the Lighting page is editing. */
export type Target = { kind: 'desk' } | { kind: 'devices'; ids: string[] } | { kind: 'lights' };

const zoneMask = (m: Mask) => m.kind === 'devices' || m.kind === 'lights' || m.kind === 'keys';

/** A plain effect, or a Studio the Lighting page can show as zones (anything else belongs to Studio). */
export function zonable(config: Config): boolean {
	const e = config.effect;
	if (e.kind !== 'studio') return true;
	const [base, ...rest] = e.layers;
	const plain = (l: StudioLayer) => l.enabled && l.opacity >= 1;
	return !!base && base.mask.kind === 'all' && plain(base) && rest.every((l) => plain(l) && zoneMask(l.mask));
}

/** The whole-desk effect. */
export const baseOf = (config: Config): LayerEffect => (config.effect.kind === 'studio' ? config.effect.layers[0].effect : config.effect);

/** The zone layers, top last. */
export const zonesOf = (config: Config): StudioLayer[] => (config.effect.kind === 'studio' ? config.effect.layers.slice(1) : []);

const sameIds = (a: string[], b: string[]) => a.length === b.length && a.every((x) => b.includes(x));
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** The zone layer for a target (not the desk), if there is one. */
export function zoneFor(config: Config, t: Target): StudioLayer | undefined {
	if (t.kind === 'desk') return undefined;
	return zonesOf(config).find((l) => (t.kind === 'lights' ? l.mask.kind === 'lights' || l.mask.kind === 'keys' : l.mask.kind === 'devices' && sameIds(l.mask.ids, t.ids)));
}

/** The effect a device shows: its own zone's, else the desk's. */
export function effectOn(config: Config, id: string): LayerEffect {
	const own = zonesOf(config).findLast((l) => l.mask.kind === 'devices' && l.mask.ids.includes(id));
	return own?.effect ?? baseOf(config);
}

/**
 * The effect to edit for a target, making its zone if it has none yet (a copy of what those devices show now,
 * so nothing changes until an effect is picked). `name` gives a device's name for the layer's label.
 */
export function editFor(config: Config, t: Target, name: (id: string) => string): { get: () => LayerEffect; set: (e: LayerEffect) => void } {
	if (t.kind === 'desk') {
		return {
			get: () => baseOf(config),
			set: (e) => {
				if (config.effect.kind === 'studio') config.effect.layers[0].effect = e;
				else config.effect = e;
			}
		};
	}
	let layer = zoneFor(config, t);
	if (!layer) {
		if (config.effect.kind !== 'studio') config.effect = { kind: 'studio', layers: [{ name: 'Whole desk', enabled: true, opacity: 1, effect: config.effect, mask: { kind: 'all' } }] };
		const layers = config.effect.layers;
		const from = t.kind === 'devices' ? effectOn(config, t.ids[0]) : baseOf(config);
		const made: StudioLayer = {
			name: t.kind === 'lights' ? 'Picked lights' : t.ids.map(name).join(', '),
			enabled: true,
			opacity: 1,
			effect: structuredClone($state.snapshot(from)) as LayerEffect,
			mask: t.kind === 'lights' ? { kind: 'lights', lights: [] } : { kind: 'devices', ids: [...t.ids] }
		};
		// A zone for several devices replaces the zones of devices it now covers.
		if (t.kind === 'devices') {
			for (let i = layers.length - 1; i >= 1; i--) {
				const m = layers[i].mask;
				if (m.kind === 'devices' && m.ids.every((id) => t.ids.includes(id))) layers.splice(i, 1);
			}
		}
		layers.push(made);
		layer = layers[layers.length - 1];
	}
	const l = layer;
	return {
		get: () => l.effect,
		set: (e) => {
			l.effect = e;
		}
	};
}

/** Give a device (or devices) back to the desk's effect. */
export function dropZone(config: Config, t: Target) {
	if (config.effect.kind !== 'studio' || t.kind === 'desk') return;
	const l = zoneFor(config, t);
	if (l) config.effect.layers.splice(config.effect.layers.indexOf(l), 1);
	tidy(config);
}

/** Remove zones that add nothing (no lights picked, or the same effect as the desk), and collapse a Studio
 * with no zones back to a plain effect. `keep` is spared (the zone being edited). */
export function tidy(config: Config, keep?: StudioLayer) {
	if (config.effect.kind !== 'studio' || !zonable(config)) return;
	const layers = config.effect.layers;
	const base = layers[0].effect;
	for (let i = layers.length - 1; i >= 1; i--) {
		const l = layers[i];
		if (l === keep) continue;
		const empty = (l.mask.kind === 'lights' || l.mask.kind === 'keys') && !lightsOf(l.mask).length;
		if (empty || same($state.snapshot(l.effect), $state.snapshot(base))) layers.splice(i, 1);
	}
	if (layers.length === 1) config.effect = layers[0].effect;
}
