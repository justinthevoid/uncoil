// Connected devices as the control pipe reports them, shared by the Keys and Hardware screens.
import { daemon, DaemonError } from './api';
import type { DeviceInfo, Feature } from './types';
import { EXPERIMENTAL, EXPERIMENTAL_TEXT } from './checks';

export const pipe = $state({
	devices: [] as DeviceInfo[],
	loaded: false,
	loading: false,
	/** uncoild isn't answering on the pipe (not running, or a build from before the control pipe). */
	unreachable: false,
	error: null as string | null
});

export async function loadDevices() {
	pipe.loading = true;
	try {
		pipe.devices = await daemon<DeviceInfo[]>('devices');
		pipe.unreachable = false;
		pipe.error = null;
	} catch (e) {
		pipe.devices = [];
		pipe.unreachable = e instanceof DaemonError && e.unreachable;
		pipe.error = String(e instanceof Error ? e.message : e);
	} finally {
		pipe.loading = false;
		pipe.loaded = true;
	}
}

const ORDER: Record<string, number> = { keyboard: 0, mouse: 1, mousemat: 2, headset: 3 };

/** Connected devices with a feature, keyboard first. */
export const withFeature = (f: Feature) =>
	pipe.devices.filter((d) => d.features.includes(f)).sort((a, b) => (ORDER[a.kind] ?? 9) - (ORDER[b.kind] ?? 9));

/** "Razer BlackWidow V4 Pro 75%" → "BlackWidow V4 Pro 75%". */
export const shortName = (name: string) => name.replace(/^Razer /, '');

/** Error codes the engine may put in front of a message (`left_click_guard: This would leave…`). */
export type ErrorCode = 'check_failed' | 'left_click_guard' | 'not_supported';
const CODE = /^(?:Error:\s*)?(check_failed|left_click_guard|not_supported):\s*/;

/** The message to show, without a leading error code. */
export const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e)).replace(CODE, '');

/** The engine's error code, from the `code:` prefix (or, failing that, the message's own words). */
export function errorCode(e: unknown): ErrorCode | null {
	const msg = e instanceof Error ? e.message : String(e);
	const m = CODE.exec(msg);
	if (m) return m[1] as ErrorCode;
	if (/no button that left-clicks/i.test(msg)) return 'left_click_guard';
	return null;
}

/** Experimental devices: set up from OpenRazer/OpenRGB data, not yet confirmed on real hardware. */
export const isExperimental = (id: string) => pipe.devices.find((d) => d.id === id)?.support === 'experimental';

/** The page-title tag for an experimental device (null for supported ones). */
export const experimentalBadge = (id: string) => (isExperimental(id) ? { text: EXPERIMENTAL, title: EXPERIMENTAL_TEXT } : null);
