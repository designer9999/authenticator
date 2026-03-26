import { invoke } from '@tauri-apps/api/core';

/**
 * @typedef {{ id: string, issuer: string, name: string, code: string, digits: number, period: number, remaining: number, created_at: number }} AccountCode
 */

/** @type {AccountCode[]} */
export let accounts = $state([]);
export let search = $state({ q: '' });
export let loading = $state({ on: true });
export let remaining = $state({ secs: 30 });

/** @type {string|null} track previous codes to detect changes */
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
			accounts.length = 0;
			data.forEach((a) => accounts.push(a));
		} else {
			data.forEach((d, i) => {
				if (accounts[i]) accounts[i].remaining = d.remaining;
			});
		}
	} catch (e) {
		console.error('Failed to load accounts:', e);
	} finally {
		loading.on = false;
		refreshing = false;
	}
}

/** Force refresh — bypasses the refreshing guard */
async function forceRefresh() {
	refreshing = false;
	prevCodes = '';
	await refresh();
}

/**
 * @param {string} issuer
 * @param {string} name
 * @param {string} secret
 */
export async function addAccount(issuer, name, secret) {
	await invoke('add_account', { issuer, name, secret });
	await forceRefresh();
}

/** @param {string} id */
export async function removeAccount(id) {
	// Remove from local array immediately (optimistic update)
	const idx = accounts.findIndex((a) => a.id === id);
	if (idx !== -1) accounts.splice(idx, 1);
	prevCodes = '';

	// Then persist to Rust
	await invoke('remove_account', { id });
}

/**
 * @param {string} id
 * @param {string} issuer
 * @param {string} name
 * @param {string} [secret]
 */
export async function editAccount(id, issuer, name, secret) {
	await invoke('edit_account', { id, issuer, name, secret: secret || null });
	await forceRefresh();
}

/**
 * Bulk import from text (one account per line: name:pass:secret)
 * @param {string} text
 * @returns {Promise<number>} count of imported accounts
 */
export async function bulkImport(text) {
	/** @type {number} */
	const count = await invoke('bulk_import', { text });
	if (count > 0) await forceRefresh();
	return count;
}

/** @param {string[]} ids */
export async function reorderAccounts(ids) {
	await invoke('reorder_accounts', { ids });
	await forceRefresh();
}
