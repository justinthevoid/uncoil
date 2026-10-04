// App-wide state: the config being edited and the daemon's last reported status.
import { getConfig, getStatus, saveConfig } from './api';
import type { Config, Status, OpenRgbDevice } from './types';

export const app = $state({
	config: null as Config | null,
	status: null as Status | null,
	/** True once the first status poll has answered, so we don't flash "not running" on launch. */
	statusKnown: false,
	saveError: null as string | null,
	loadError: null as string | null
});

/** Ids of the devices the daemon reports as connected, sorted (for the desk and the preview). */
export const connectedIds = (): string[] => (app.status?.devices.map((d) => d.id) ?? []).toSorted();

/** What the desk depends on besides the config: connected device ids and the devices OpenRGB reports.
 * `key` changes when either changes, so the desk is fetched again only then. */
export function deskSources() {
	const connected = connectedIds();
	const external = $state.snapshot(app.status?.openrgb?.devices ?? []) as OpenRgbDevice[];
	return { connected, external, key: `${connected.join(',')}|${external.map((d) => `${d.id}:${d.leds}`).join(',')}` };
}

export async function loadConfig() {
	try {
		app.config = await getConfig();
	} catch (e) {
		app.loadError = String(e);
	}
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

/** Persist the config ~300 ms after the last change; the daemon hot-reloads the file. */
export function scheduleSave(config: Config) {
	clearTimeout(saveTimer);
	saveTimer = setTimeout(async () => {
		try {
			await saveConfig(config);
			app.saveError = null;
		} catch (e) {
			app.saveError = String(e);
		}
	}, 300);
}

export async function pollStatus() {
	try {
		app.status = await getStatus();
	} catch {
		app.status = null;
	}
	app.statusKnown = true;
}
