// Base-aware internal links, so the site works at /uncoil/ on GitHub Pages and at / on a custom domain.
const base = import.meta.env.BASE_URL.replace(/\/+$/, '');

export const url = (path = '') => `${base}/${path.replace(/^\/+/, '')}`;

export const REPO = 'https://github.com/justinthevoid/uncoil';
/** TODO(release): point at the first published release once one exists. */
export const RELEASES = `${REPO}/releases`;
export const blob = (path: string) => `${REPO}/blob/main/${path}`;
