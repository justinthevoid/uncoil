// One name per page, used by the rail, the page title and the "engine isn't answering" notice, so they
// can't drift apart.

export type PageId = 'lighting' | 'studio' | 'pc' | 'devices' | 'keys' | 'buttons' | 'performance' | 'power' | 'dial' | 'effects' | 'info' | 'settings' | 'app' | 'about';

export interface Page {
	/** Rail label, and the page title unless `title` says otherwise. */
	label: string;
	title?: string;
	/** How a sentence names what the page does ("Key remapping talks to your devices…"); default `label`. */
	subject?: string;
}

export const PAGES: Record<PageId, Page> = {
	lighting: { label: 'Lighting' },
	studio: { label: 'Studio' },
	pc: { label: 'PC', title: 'Inside the PC', subject: 'PC lighting' },
	devices: { label: 'Devices' },
	keys: { label: 'Keys', subject: 'Key remapping' },
	buttons: { label: 'Buttons', subject: 'Button remapping' },
	performance: { label: 'Performance' },
	power: { label: 'Battery & sleep' },
	dial: { label: 'Dial & screen' },
	effects: { label: 'Onboard effects' },
	info: { label: 'Device info' },
	settings: { label: 'Display & RGB' },
	app: { label: 'Tray & notifications' },
	about: { label: 'About', title: 'About uncoil' }
};

export const pageTitle = (id: PageId) => PAGES[id].title ?? PAGES[id].label;
export const pageSubject = (id: PageId) => PAGES[id].subject ?? PAGES[id].label;
