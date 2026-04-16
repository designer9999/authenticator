import { invoke } from '@tauri-apps/api/core';

/**
 * @typedef {{ id: string, issuer: string, name: string, code: string, digits: number, period: number, remaining: number, created_at: number }} AccountCode
 */

/** @type {AccountCode[]} */
export let accounts = $state([]);

// Object wrappers for exported $state — Svelte 5 requires objects for
// module-level reactive exports (primitives lose reactivity across imports)
export let search = $state({ q: '' });
export let loading = $state({ on: true });
export let remaining = $state({ secs: 30 });

let prevCodes = '';
let refreshing = false;

export async function refresh() {
	if (refreshing) return;
	refreshing = true;
	try {
		/** @type {AccountCode[]} */
		const data = await invoke('get_accounts');

		if (data.length > 0) {
			remaining.secs = data[0].remaining;
		} else {
			remaining.secs = 30 - (Math.floor(Date.now() / 1000) % 30);
		}

		const newCodes = data.map((d) => d.id + d.code).join(',');
		if (newCodes !== prevCodes) {
			prevCodes = newCodes;
			accounts.splice(0, accounts.length, ...data);
		} else {
			for (let i = 0; i < data.length; i++) {
				if (accounts[i]) accounts[i].remaining = data[i].remaining;
			}
		}
	} catch {
		// Silently retry on next interval
	} finally {
		loading.on = false;
		refreshing = false;
	}
}

async function forceRefresh() {
	refreshing = false;
	prevCodes = '';
	await refresh();
}

/**
 * @param {string} issuer
 * @param {string} name
 * @param {string | undefined} password
 * @param {string} secret
 */
export async function addAccount(issuer, name, password, secret) {
	await invoke('add_account', { issuer, name, password: password || null, secret });
	await forceRefresh();
}

/** @param {string} id */
export async function removeAccount(id) {
	const backup = [...accounts];
	const idx = accounts.findIndex((a) => a.id === id);
	if (idx !== -1) accounts.splice(idx, 1);
	prevCodes = '';

	try {
		await invoke('remove_account', { id });
	} catch {
		// Rollback on failure
		accounts.splice(0, accounts.length, ...backup);
		prevCodes = '';
	}
}

/**
 * @param {string} id
 * @param {string} issuer
 * @param {string} name
 * @param {string | undefined} password
 * @param {string | undefined} secret
 */
export async function editAccount(id, issuer, name, password, secret) {
	await invoke('edit_account', {
		id,
		issuer,
		name,
		password: password ?? null,
		secret: secret || null,
	});
	await forceRefresh();
}

/** @param {string[]} ids */
export async function reorderAccounts(ids) {
	await invoke('reorder_accounts', { ids });
	await forceRefresh();
}

/**
 * @param {string} text
 * @returns {Promise<number>}
 */
export async function bulkImport(text) {
	/** @type {number} */
	const count = await invoke('bulk_import', { text });
	if (count > 0) await forceRefresh();
	return count;
}
