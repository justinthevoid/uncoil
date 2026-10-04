// Experimental devices: the read-only checks that must pass before uncoil changes settings stored on them,
// and the fixed words and links that go with them.
import type { Capabilities, CheckState, DeviceKind, Feature, FeatureCheck } from './types';

export const REPO = 'https://github.com/justinthevoid/uncoil';
export const REPORT_URL = `${REPO}/issues/new?template=device_report.yml`;
export const SUPPORT_URL = `${REPO}/issues/new?template=device_support.yml`;
/** The configuration guide (the site's source, so the opener's GitHub-only permission covers it). */
export const CONFIG_DOC_URL = `${REPO}/blob/main/site/src/content/docs/docs/configuration.md#openrgb`;

export const EXPERIMENTAL = 'Experimental';
export const EXPERIMENTAL_TEXT = 'Set up from OpenRazer and OpenRGB data. Nobody has confirmed it on this device yet.';

/** "0x00B6". */
export const productId = (n: number) => '0x' + n.toString(16).toUpperCase().padStart(4, '0');

export const FEATURE_NAMES: Record<Feature, string> = {
	lighting: 'Lighting',
	hw_effects: 'Onboard effects',
	keymap: 'Key remapping',
	profiles: 'Onboard profiles',
	dial: 'Command dial',
	oled: 'Screen',
	dpi: 'DPI',
	poll_rate: 'Polling rate',
	power: 'Battery and sleep',
	scroll: 'Scroll wheel'
};

export const STATE_NAMES: Record<CheckState, string> = {
	passed: 'Passed',
	failed: 'Failed',
	untested: 'Not run yet',
	not_needed: 'Not needed'
};

/** What each check reads, in plain words ("It reads …"). */
export function checkReads(feature: Feature, kind: DeviceKind): string {
	switch (feature) {
		case 'keymap':
			return kind === 'mouse' ? 'what the left and right buttons do' : 'what P, A and Esc do';
		case 'profiles':
			return 'the list of onboard profiles';
		case 'dpi':
			return 'the current DPI';
		case 'poll_rate':
			return 'the polling rate';
		case 'power':
			return 'the battery level and sleep settings';
		case 'scroll':
			return 'the scroll wheel settings';
		case 'lighting':
		case 'hw_effects':
			return 'the lighting layout the device reports';
		default:
			return 'a current setting';
	}
}

export const checkOf = (caps: Capabilities | null, f: Feature): FeatureCheck | null => caps?.checks?.find((c) => c.feature === f) ?? null;

/** Writes for this feature are refused until its check passes. */
export const locked = (caps: Capabilities | null, f: Feature) => {
	const s = checkOf(caps, f)?.state;
	return s === 'untested' || s === 'failed';
};

/** "a, b and c". */
export const joinWords = (xs: string[]) => (xs.length <= 1 ? (xs[0] ?? '') : `${xs.slice(0, -1).join(', ')} and ${xs[xs.length - 1]}`);
