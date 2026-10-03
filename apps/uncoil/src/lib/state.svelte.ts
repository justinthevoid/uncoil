// App-wide state: the config being edited and the daemon's last reported status.
import { getConfig, getStatus, saveConfig } from './api';
import type { Config, Status } from './types';

export const app = $state({
	config: null as Config | null,
	status: null as Status | null,
	/** True once the first status poll has answered, so we don't flash "not running" on launch. */
	statusKnown: false,
	saveError: null as string | null,
	loadError: null as string | null
});

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
