<script>
	import { invoke } from '@tauri-apps/api/core';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { slide, fade } from 'svelte/transition';
	import { flip } from 'svelte/animate';
	import { cubicOut } from 'svelte/easing';
	import IconBtn from '$lib/IconBtn.svelte';
	import IssuerPicker from '$lib/IssuerPicker.svelte';
	import { open } from '@tauri-apps/plugin-dialog';
	import {
		accounts,
		search,
		loading,
		remaining,
		refresh,
		addAccount,
		removeAccount,
		editAccount,
		// reorderAccounts, // TODO: for mobile touch reorder
		bulkImport,
	} from '$lib/state.svelte.js';
	import { getBrand } from '$lib/brands.js';

	// ── Window controls ──
	const win = getCurrentWindow();
	async function windowDrag() {
		await win.startDragging();
	}
	async function windowToggleMax() {
		await win.toggleMaximize();
	}
	async function windowMin() {
		await win.minimize();
	}
	async function windowClose() {
		await win.close();
	}

	/** @param {MouseEvent} e */
	function handleTitlebarMouseDown(e) {
		if (e.button !== 0) return;
		const t = /** @type {HTMLElement} */ (e.target);
		if (t.closest('button, input, a, [role="button"]')) return;
		if (e.detail === 2) {
			windowToggleMax();
		} else {
			windowDrag();
		}
	}

	// ── State ──
	let showAdd = $state(false);
	let showEdit = $state(false);
	let showDelete = $state(false);
	let showSettings = $state(false);
	let deleteId = $state('');
	let editId = $state('');
	let editIssuer = $state('');
	let editName = $state('');
	let editSecret = $state('');
	let copied = $state('');
	let copiedTimer = 0;
	let quickIssuerChangeId = $state('');
	let quickPaste = $state('');
	let issuer = $state('');
	let name = $state('');
	let secret = $state('');
	let error = $state('');

	/** @type {{ version: string, account_count: number, data_path: string } | null} */
	let appInfo = $state(null);

	let filterIssuer = $state('');

	let issuers = $derived([...new Set(accounts.map((a) => a.issuer))]);

	let filtered = $derived(
		accounts.filter((a) => {
			if (filterIssuer && a.issuer !== filterIssuer) return false;
			if (!search.q) return true;
			const q = search.q.toLowerCase();
			return a.issuer.toLowerCase().includes(q) || a.name.toLowerCase().includes(q);
		})
	);

	let timer = $derived((remaining.secs / 30) * 100);

	$effect(() => {
		refresh();
		const id = setInterval(refresh, 1000);
		return () => clearInterval(id);
	});

	/** @param {number} ms — timestamp in milliseconds */
	function timeAgo(ms) {
		if (!ms) return '';
		const secs = Math.floor((Date.now() - ms) / 1000);
		if (secs < 60) return 'just now';
		const mins = Math.floor(secs / 60);
		if (mins < 60) return `${mins}m ago`;
		const hrs = Math.floor(mins / 60);
		if (hrs < 24) return `${hrs}h ago`;
		const days = Math.floor(hrs / 24);
		if (days < 30) return `${days}d ago`;
		const months = Math.floor(days / 30);
		if (months < 12) return `${months}mo ago`;
		return `${Math.floor(months / 12)}y ago`;
	}

	/** @param {string} code */
	function fmt(code) {
		return code.length === 6 ? code.slice(0, 3) + ' ' + code.slice(3) : code;
	}

	/**
	 * @param {string} id
	 * @param {string} code
	 */
	async function copy(id, code) {
		try {
			await navigator.clipboard.writeText(code.replace(/\s/g, ''));
			copied = id;
			clearTimeout(copiedTimer);
			copiedTimer = setTimeout(() => (copied = ''), 2000);
		} catch {
			/* noop */
		}
	}

	function openAdd() {
		quickPaste = '';
		issuer = '';
		name = '';
		secret = '';
		error = '';
		showAdd = true;
	}

	function handleQuickPaste() {
		const val = quickPaste.trim();
		if (!val) return;
		const parts = val.split(':');
		if (parts.length >= 3) {
			name = parts[0].trim();
			secret = parts.slice(2).join('').trim();
		} else if (parts.length === 2) {
			name = parts[0].trim();
			secret = parts[1].trim();
		} else {
			secret = val;
		}
		quickPaste = '';
	}

	async function handleAdd() {
		if (!name.trim() || !secret.trim()) {
			error = 'Account name and secret key are required';
			return;
		}
		try {
			error = '';
			const finalIssuer = issuer.trim() || name.trim().charAt(0).toUpperCase();
			await addAccount(finalIssuer, name.trim(), secret.trim());
			showAdd = false;
		} catch (e) {
			error = String(e);
		}
	}

	async function confirmDelete() {
		showDelete = false;
		await removeAccount(deleteId);
	}

	/**
	 * @param {string} accountId
	 * @param {string} newIssuer
	 */
	async function quickChangeIssuer(accountId, newIssuer) {
		const acc = accounts.find((a) => a.id === accountId);
		if (!acc) return;
		await editAccount(accountId, newIssuer, acc.name);
		quickIssuerChangeId = '';
	}

	/**
	 * @param {string} id
	 * @param {string} currentIssuer
	 * @param {string} currentName
	 */
	/**
	 * @param {string} id
	 * @param {string} currentIssuer
	 * @param {string} currentName
	 */
	function openEdit(id, currentIssuer, currentName) {
		editId = id;
		editIssuer = currentIssuer;
		editName = currentName;
		editSecret = '';
		error = '';
		showEdit = true;
	}

	async function handleEdit() {
		if (!editIssuer.trim() || !editName.trim()) {
			error = 'Issuer and name are required';
			return;
		}
		try {
			error = '';
			await editAccount(editId, editIssuer.trim(), editName.trim(), editSecret.trim() || undefined);
			showEdit = false;
		} catch (e) {
			error = String(e);
		}
	}

	async function openSettings() {
		appInfo = await invoke('get_app_info');
		showSettings = true;
	}

	async function openDataFolder() {
		await invoke('open_data_folder');
	}

	async function changeDataPath() {
		const selected = await open({ directory: true, title: 'Choose data folder' });
		if (!selected) return;
		await invoke('change_data_path', { newPath: selected });
		appInfo = await invoke('get_app_info');
		await refresh();
	}

	// ── Drag-and-drop file import (Tauri native API) ──
	let dropActive = $state(false);
	let importMsg = $state('');
	let importTimer = 0;

	$effect(() => {
		const promise = /** @type {any} */ (win).onDragDropEvent(
			/** @param {{ payload: { type: string, paths?: string[] } }} event */
			(event) => {
				const { type } = event.payload;
				if (type === 'over') {
					dropActive = true;
				} else if (type === 'leave') {
					dropActive = false;
				} else if (type === 'drop') {
					dropActive = false;
					const paths = event.payload.paths;
					if (paths && paths.length > 0) {
						handleFileDrop(paths[0]);
					}
				}
			}
		);
		return () => {
			promise.then(/** @param {Function} fn */ (fn) => fn());
		};
	});

	/** @param {string} path */
	async function handleFileDrop(path) {
		try {
			/** @type {string} */
			const text = await invoke('read_text_file', { path });
			const count = await bulkImport(text);
			importMsg =
				count > 0 ? `Imported ${count} account${count > 1 ? 's' : ''}` : 'No valid accounts found';
			clearTimeout(importTimer);
			importTimer = setTimeout(() => (importMsg = ''), 3000);
		} catch {
			importMsg = 'Failed to read file';
			clearTimeout(importTimer);
			importTimer = setTimeout(() => (importMsg = ''), 3000);
		}
	}

	async function handleImportFile() {
		const selected = await open({
			title: 'Import accounts file',
			filters: [{ name: 'Text', extensions: ['txt', 'csv'] }],
		});
		if (!selected) return;
		/** @type {string} */
		const text = await invoke('read_text_file', { path: selected });
		const count = await bulkImport(text);
		importMsg =
			count > 0 ? `Imported ${count} account${count > 1 ? 's' : ''}` : 'No valid accounts found';
		clearTimeout(importTimer);
		importTimer = setTimeout(() => (importMsg = ''), 3000);
	}
</script>

<div class="flex h-screen flex-col overflow-hidden bg-surface" role="application">
	<!-- Drop zone overlay -->
	{#if dropActive}
		<div class="drop-overlay">
			<div class="drop-card">
				<span class="material-symbols-outlined text-5xl text-primary">upload_file</span>
				<p class="text-lg font-medium text-on-surface">Drop file to import accounts</p>
				<p class="text-sm text-on-surface-variant">One account per line: name:pass:secret</p>
			</div>
		</div>
	{/if}

	<!-- Import success message -->
	{#if importMsg}
		<div class="import-toast">{importMsg}</div>
	{/if}

	<!-- ═══ Titlebar ═══ -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="titlebar" onmousedown={handleTitlebarMouseDown}>
		<div class="flex items-center gap-2">
			<span class="material-symbols-outlined titlebar-app-icon text-primary">shield</span>
			<span class="text-xs font-medium text-on-surface-variant">Authenticator</span>
		</div>
		<div class="flex items-center gap-0.5 pr-1.5">
			<button class="tb-btn" onclick={windowMin} aria-label="Minimize" title="Minimize">
				<span class="material-symbols-outlined">remove</span>
			</button>
			<button class="tb-btn" onclick={windowToggleMax} aria-label="Maximize" title="Maximize">
				<span class="material-symbols-outlined">crop_square</span>
			</button>
			<button class="tb-btn tb-close" onclick={windowClose} aria-label="Close" title="Close">
				<span class="material-symbols-outlined">close</span>
			</button>
		</div>
	</div>

	<!-- ═══ App Bar ═══ -->
	<header class="flex h-16 shrink-0 items-center gap-4 px-6">
		<!-- M3 Search: 56dp, pill, surfaceContainerHigh, elevation 3 -->
		<div class="search-bar">
			<span class="material-symbols-outlined text-2xl text-on-surface-variant">search</span>
			<input
				type="text"
				placeholder="Search..."
				bind:value={search.q}
				autocomplete="off"
				class="flex-1 border-none bg-transparent text-base text-on-surface outline-none placeholder:text-on-surface-variant"
			/>
		</div>
		<!-- M3 Icon Button: 40px circle, 8% hover, FILL on press -->
		<IconBtn icon="settings" label="Settings" onclick={openSettings} />
	</header>

	<!-- ═══ M3 Linear Progress (determinate) ═══ -->
	<!-- Spec: 4dp track, primary active, secondaryContainer track, 4dp inset -->
	<div class="progress-track" aria-hidden="true">
		<div
			class="progress-fill"
			class:progress-reset={remaining.secs >= 29}
			style="transform: scaleX({timer / 100})"
		></div>
	</div>

	<!-- ═══ Account List ═══ -->
	<main class="flex-1 overflow-y-auto thin-scrollbar">
		{#if loading.on}
			<div class="flex h-full items-center justify-center">
				<span class="material-symbols-outlined animate-spin text-5xl text-on-surface-variant">
					progress_activity
				</span>
			</div>
		{:else if filtered.length === 0}
			<div
				class="flex h-full flex-col items-center justify-center gap-4 px-6 text-on-surface-variant"
			>
				<span class="material-symbols-outlined text-7xl opacity-30">shield</span>
				<h2 class="text-xl font-medium text-on-surface">No accounts yet</h2>
				<p class="text-sm opacity-60">Tap + to add your first account</p>
			</div>
		{:else}
			{#if issuers.length > 1}
				<div class="filter-bar">
					<button
						class="filter-chip"
						class:active={!filterIssuer}
						onclick={() => (filterIssuer = '')}>All</button
					>
					{#each issuers as iss (iss)}
						<button
							class="filter-chip"
							class:active={filterIssuer === iss}
							onclick={() => (filterIssuer = filterIssuer === iss ? '' : iss)}>{iss}</button
						>
					{/each}
				</div>
			{/if}

			{#each filtered as account (account.id)}
				{@const brand = getBrand(account.issuer)}
				<div
					class="account-row"
					draggable="false"
					in:fade|local={{ duration: 200, easing: cubicOut }}
					out:slide|local={{ duration: 200, axis: 'y' }}
					animate:flip={{ duration: 250, easing: cubicOut }}
					onclick={() => {
						quickIssuerChangeId = '';
						copy(account.id, account.code);
					}}
					onkeydown={(e) => e.key === 'Enter' && copy(account.id, account.code)}
					role="button"
					tabindex="0"
				>
					<button
						class="brand-icon"
						onclick={(e) => {
							e.stopPropagation();
							quickIssuerChangeId = quickIssuerChangeId === account.id ? '' : account.id;
						}}
						title="Change issuer"
					>
						{#if brand.svg}
							<!-- eslint-disable-next-line svelte/no-at-html-tags -->
							{@html brand.svg}
						{:else}
							{brand.letter}
						{/if}
					</button>
					{#if quickIssuerChangeId === account.id}
						<div class="quick-picker" transition:slide={{ duration: 200, axis: 'y' }}>
							<IssuerPicker
								selected={account.issuer}
								onselect={(name) => quickChangeIssuer(account.id, name)}
							/>
						</div>
					{/if}

					<div class="min-w-0 flex-1">
						<div class="truncate text-sm text-on-surface">{account.issuer}: {account.name}</div>
						<div class="code-text">{fmt(account.code)}</div>
						{#if account.created_at}
							<div class="text-xs text-on-surface-variant opacity-50">
								{timeAgo(account.created_at)}
							</div>
						{/if}
					</div>

					<div class="row-actions">
						<IconBtn
							icon="edit"
							label="Edit"
							class="hover-action"
							onclick={(e) => {
								e.stopPropagation();
								openEdit(account.id, account.issuer, account.name);
							}}
						/>
						<IconBtn
							icon="delete"
							label="Delete"
							class="hover-action del-btn"
							onclick={(e) => {
								e.stopPropagation();
								deleteId = account.id;
								showDelete = true;
							}}
						/>
						<IconBtn
							icon={copied === account.id ? 'check' : 'content_copy'}
							label="Copy code"
							class="copy-btn {copied === account.id ? 'copied' : ''}"
							onclick={(e) => {
								e.stopPropagation();
								copy(account.id, account.code);
							}}
						/>
					</div>
				</div>
			{/each}
		{/if}
	</main>

	<!-- FAB -->
	<button class="fab" onclick={openAdd} aria-label="Add account" title="Add account">
		<span class="material-symbols-outlined text-2xl">add</span>
	</button>

	<!-- ═══ Add Dialog ═══ -->
	{#if showAdd}
		<div
			class="dialog-scrim"
			onclick={() => (showAdd = false)}
			onkeydown={(e) => e.key === 'Escape' && (showAdd = false)}
			role="presentation"
		>
			<div
				class="dialog-container"
				onclick={(e) => e.stopPropagation()}
				onkeydown={(e) => e.key === 'Escape' && (showAdd = false)}
				role="alertdialog"
				aria-label="Add account"
				tabindex="-1"
			>
				<h2 class="dialog-title">Add account</h2>
				<div class="m3-field">
					<input
						type="text"
						placeholder=" "
						bind:value={quickPaste}
						oninput={handleQuickPaste}
						id="f-quick"
					/>
					<label for="f-quick">Quick paste (name:pass:secret)</label>
				</div>
				<button
					class="m3-btn-tonal"
					onclick={() => {
						showAdd = false;
						handleImportFile();
					}}
				>
					<span class="material-symbols-outlined text-lg">upload_file</span>
					Import from file
				</button>
				<div class="divider"></div>

				<!-- Issuer icon picker -->
				<IssuerPicker selected={issuer} onselect={(name) => (issuer = name)} />

				<div class="m3-field">
					<input type="text" placeholder=" " autocomplete="off" bind:value={issuer} id="f-issuer" />
					<label for="f-issuer">Or type issuer name</label>
				</div>
				<div class="m3-field">
					<input type="text" placeholder=" " autocomplete="off" bind:value={name} id="f-name" />
					<label for="f-name">Account name</label>
				</div>
				<div class="m3-field">
					<input type="text" placeholder=" " autocomplete="off" bind:value={secret} id="f-secret" />
					<label for="f-secret">Secret key (base32)</label>
				</div>
				{#if error}<p class="error-msg">{error}</p>{/if}
				<div class="dialog-actions">
					<button class="m3-btn-text" onclick={() => (showAdd = false)}>Cancel</button>
					<button class="m3-btn-filled" onclick={handleAdd}>Add</button>
				</div>
			</div>
		</div>
	{/if}

	<!-- ═══ Edit Dialog ═══ -->
	{#if showEdit}
		<div
			class="dialog-scrim"
			onclick={() => (showEdit = false)}
			onkeydown={(e) => e.key === 'Escape' && (showEdit = false)}
			role="presentation"
		>
			<div
				class="dialog-container"
				onclick={(e) => e.stopPropagation()}
				onkeydown={(e) => e.key === 'Escape' && (showEdit = false)}
				role="alertdialog"
				aria-label="Edit account"
				tabindex="-1"
			>
				<h2 class="dialog-title">Edit account</h2>
				<IssuerPicker selected={editIssuer} onselect={(name) => (editIssuer = name)} />
				<div class="m3-field">
					<input
						type="text"
						placeholder=" "
						autocomplete="off"
						bind:value={editIssuer}
						id="f-edit-issuer"
					/>
					<label for="f-edit-issuer">Or type issuer name</label>
				</div>
				<div class="m3-field">
					<input
						type="text"
						placeholder=" "
						autocomplete="off"
						bind:value={editName}
						id="f-edit-name"
					/>
					<label for="f-edit-name">Account name</label>
				</div>
				<div class="m3-field">
					<input
						type="text"
						placeholder=" "
						autocomplete="off"
						bind:value={editSecret}
						id="f-edit-secret"
					/>
					<label for="f-edit-secret">New secret key (leave empty to keep current)</label>
				</div>
				{#if error}<p class="error-msg">{error}</p>{/if}
				<div class="dialog-actions">
					<button class="m3-btn-text" onclick={() => (showEdit = false)}>Cancel</button>
					<button class="m3-btn-filled" onclick={handleEdit}>Save</button>
				</div>
			</div>
		</div>
	{/if}

	<!-- ═══ Delete Dialog ═══ -->
	{#if showDelete}
		<div
			class="dialog-scrim"
			onclick={() => (showDelete = false)}
			onkeydown={(e) => e.key === 'Escape' && (showDelete = false)}
			role="presentation"
		>
			<div
				class="dialog-container"
				onclick={(e) => e.stopPropagation()}
				onkeydown={(e) => e.key === 'Escape' && (showDelete = false)}
				role="alertdialog"
				aria-label="Confirm delete"
				tabindex="-1"
			>
				<h2 class="dialog-title">Remove account?</h2>
				<p class="dialog-body">This will permanently remove this authenticator entry.</p>
				<div class="dialog-actions">
					<button class="m3-btn-text" onclick={() => (showDelete = false)}>Cancel</button>
					<button class="m3-btn-filled m3-btn-error" onclick={confirmDelete}>Remove</button>
				</div>
			</div>
		</div>
	{/if}

	<!-- ═══ Settings ═══ -->
	{#if showSettings}
		<div
			class="dialog-scrim"
			onclick={() => (showSettings = false)}
			onkeydown={(e) => e.key === 'Escape' && (showSettings = false)}
			role="presentation"
		>
			<div
				class="dialog-container"
				onclick={(e) => e.stopPropagation()}
				onkeydown={(e) => e.key === 'Escape' && (showSettings = false)}
				role="dialog"
				aria-label="Settings"
				tabindex="-1"
			>
				<div class="flex items-center gap-4 mb-6">
					<span class="material-symbols-outlined icon-filled text-4xl text-primary">shield</span>
					<div>
						<h2 class="text-xl font-medium text-on-surface">Authenticator</h2>
						<p class="text-xs text-on-surface-variant">v{appInfo?.version ?? '...'}</p>
					</div>
				</div>

				<div class="settings-section">
					<div class="settings-row">
						<span class="material-symbols-outlined text-2xl text-on-surface-variant">tag</span>
						<div class="flex-1">
							<div class="text-sm text-on-surface">Version</div>
							<div class="text-xs text-on-surface-variant">{appInfo?.version ?? '...'}</div>
						</div>
					</div>
					<div class="settings-row">
						<span class="material-symbols-outlined text-2xl text-on-surface-variant">key</span>
						<div class="flex-1">
							<div class="text-sm text-on-surface">Accounts stored</div>
							<div class="text-xs text-on-surface-variant">
								{appInfo?.account_count ?? 0} accounts
							</div>
						</div>
					</div>
					<button class="settings-row settings-row-btn" onclick={openDataFolder}>
						<span class="material-symbols-outlined text-2xl text-on-surface-variant"
							>folder_open</span
						>
						<div class="flex-1 min-w-0 text-left">
							<div class="text-sm text-on-surface">Data location</div>
							<div class="truncate text-xs text-on-surface-variant">
								{appInfo?.data_path ?? '...'}
							</div>
						</div>
						<span class="material-symbols-outlined text-xl text-on-surface-variant"
							>open_in_new</span
						>
					</button>
					<button class="settings-row settings-row-btn" onclick={changeDataPath}>
						<span class="material-symbols-outlined text-2xl text-on-surface-variant"
							>drive_file_move</span
						>
						<div class="flex-1 text-left">
							<div class="text-sm text-on-surface">Change storage location</div>
							<div class="text-xs text-on-surface-variant">Move accounts to a different folder</div>
						</div>
						<span class="material-symbols-outlined text-xl text-on-surface-variant"
							>chevron_right</span
						>
					</button>
				</div>

				<div class="divider"></div>

				<div class="settings-section">
					<div class="settings-row">
						<span class="material-symbols-outlined text-2xl text-on-surface-variant">code</span>
						<div class="flex-1">
							<div class="text-sm text-on-surface">Built with</div>
							<div class="text-xs text-on-surface-variant">Tauri v2 + Svelte 5 + Rust</div>
						</div>
					</div>
					<div class="settings-row">
						<span class="material-symbols-outlined text-2xl text-on-surface-variant">security</span>
						<div class="flex-1">
							<div class="text-sm text-on-surface">Storage</div>
							<div class="text-xs text-on-surface-variant">Local JSON — handles 1000+ accounts</div>
						</div>
					</div>
				</div>

				<div class="dialog-actions">
					<button class="m3-btn-text" onclick={() => (showSettings = false)}>Close</button>
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
	/* ═══ Titlebar ═══ */
	.titlebar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 40px;
		padding-left: 12px;
		flex-shrink: 0;
		background: var(--color-surface);
		user-select: none;
		-webkit-user-select: none;
	}

	.titlebar-app-icon {
		font-size: 18px;
		font-variation-settings:
			'FILL' 1,
			'wght' 400,
			'GRAD' -25,
			'opsz' 20;
	}

	.tb-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		border: none;
		border-radius: 9999px;
		background: transparent;
		color: var(--color-on-surface-variant);
		cursor: pointer;
		transition: background var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.tb-btn .material-symbols-outlined {
		font-size: 18px;
		font-variation-settings:
			'FILL' 0,
			'wght' 300,
			'GRAD' -25,
			'opsz' 20;
		transition: font-variation-settings var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.tb-btn:hover {
		background: color-mix(in srgb, var(--color-on-surface) 8%, transparent);
	}

	.tb-btn:active .material-symbols-outlined {
		font-variation-settings:
			'FILL' 1,
			'wght' 300,
			'GRAD' -25,
			'opsz' 20;
	}

	.tb-close:hover {
		background: #e81123;
		color: white;
	}

	/* ═══ Search Bar — M3: 56dp, pill, surfaceContainerHigh, e3 ═══ */
	.search-bar {
		display: flex;
		flex: 1;
		align-items: center;
		gap: 12px;
		height: 48px;
		padding: 0 20px;
		border-radius: 9999px;
		background: var(--color-surface-container-high);
		box-shadow: var(--shadow-e3);
		transition: background var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.search-bar:focus-within {
		background: var(--color-surface-container-highest);
	}

	/* Icon button overrides for row context */

	/* ═══ M3 Linear Progress (determinate) ═══
	   Spec: 4dp track height, primary active, secondaryContainer track
	   Inset 4dp from edges, min 40dp width */
	.progress-track {
		flex-shrink: 0;
		height: 6px;
		margin: 0 4px;
		border-radius: 3px;
		background: var(--color-secondary-container);
	}

	.progress-fill {
		height: 6px;
		border-radius: 3px;
		background: var(--color-primary);
		transform-origin: left center;
		transition: transform 1s linear;
		will-change: transform;
	}

	.progress-reset {
		transition: none !important;
	}

	/* ═══ Account Row — M3 2-line list item ═══
	   Spec: leading left 16dp, label left 16dp, trailing right 24dp, height 72dp */
	.account-row {
		position: relative;
		display: flex;
		align-items: center;
		gap: 16px; /* M3: label left padding from leading element */
		width: 100%;
		min-height: 72px;
		padding: 8px 24px 8px 16px; /* M3: leading 16dp left, trailing 24dp right */
		cursor: pointer;
		border-bottom: 1px solid color-mix(in srgb, var(--color-outline-variant) 40%, transparent);
		transition: background var(--m3-fast-effects-dur) var(--m3-fast-effects);
		-webkit-user-drag: none;
		user-select: none;
	}

	.account-row:hover {
		background: color-mix(in srgb, var(--color-on-surface) 8%, transparent);
	}

	.account-row:active {
		background: color-mix(in srgb, var(--color-on-surface) 10%, transparent);
	}

	.account-row:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: -2px;
	}

	/* M3 Leading Avatar — 40dp circle, clickable to change issuer */
	.brand-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 40px;
		height: 40px;
		flex-shrink: 0;
		border: none;
		border-radius: 9999px;
		background: var(--color-tertiary-container);
		color: var(--color-on-tertiary-container);
		font-size: 0.875rem;
		font-weight: 600;
		line-height: 1;
		user-select: none;
		overflow: hidden;
		cursor: pointer;
		transition:
			transform var(--m3-fast-spatial-dur) var(--m3-fast-spatial),
			box-shadow var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.brand-icon:hover {
		transform: scale(1.1);
		box-shadow: var(--shadow-e2);
	}

	.brand-icon:active {
		transform: scale(0.95);
	}

	/* Quick issuer picker inline */
	.quick-picker {
		position: absolute;
		left: 0;
		right: 0;
		top: 100%;
		z-index: 10;
		padding: 8px 16px;
		background: var(--color-surface-container-high);
		border-radius: 0 0 16px 16px;
		box-shadow: var(--shadow-e2);
	}

	.brand-icon :global(svg) {
		width: 20px;
		height: 20px;
	}

	/* TOTP code — M3 Headline Medium: 1.75rem (28px), BOLD, primary, single line */
	.code-text {
		font-size: 1.75rem;
		font-weight: 700;
		line-height: 2.25rem;
		font-variant-numeric: tabular-nums;
		letter-spacing: 0.05em;
		color: var(--color-primary);
		white-space: nowrap;
		animation: codeFade var(--m3-default-effects-dur) var(--m3-default-effects);
	}

	@keyframes codeFade {
		from {
			opacity: 0.3;
		}
	}

	/* Svelte handles enter/exit/reorder animations via transition: and animate: directives */

	/* Trailing actions container */
	.row-actions {
		display: flex;
		align-items: center;
		gap: 2px;
		flex-shrink: 0;
		margin-left: auto;
	}

	/* Edit/delete: hidden, appear on row hover — :global because buttons are in child component */
	.row-actions :global(.hover-action) {
		opacity: 0;
		transition: opacity var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.account-row:hover .row-actions :global(.hover-action) {
		opacity: 1;
	}

	.row-actions :global(.del-btn:hover) {
		color: var(--color-error);
	}

	.row-actions :global(.copy-btn) {
		color: var(--color-primary);
	}

	.row-actions :global(.copied) {
		color: var(--color-success);
	}

	/* ═══ M3 Filter Chips — 32px, 8px radius, outline border ═══ */
	.filter-bar {
		display: flex;
		gap: 8px;
		padding: 8px 16px;
		overflow-x: auto;
		scrollbar-width: none;
	}

	.filter-chip {
		height: 32px;
		padding: 0 16px;
		border: 1px solid var(--color-outline);
		border-radius: 8px;
		background: transparent;
		color: var(--color-on-surface-variant);
		font-family: var(--font-sans);
		font-size: 0.875rem;
		font-weight: 500;
		white-space: nowrap;
		cursor: pointer;
		transition:
			background var(--m3-fast-effects-dur) var(--m3-fast-effects),
			border-color var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.filter-chip:hover {
		background: color-mix(in srgb, var(--color-on-surface) 8%, transparent);
	}

	.filter-chip.active {
		background: var(--color-secondary-container);
		color: var(--color-on-secondary-container);
		border-color: transparent;
	}

	/* ═══ FAB — M3: 56px, 16px radius, e3, hover e4 ═══ */
	.fab {
		position: fixed;
		right: 24px;
		bottom: 24px;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 56px;
		height: 56px;
		border: none;
		border-radius: 16px;
		background: var(--color-primary-container);
		color: var(--color-on-primary-container);
		box-shadow: var(--shadow-e3);
		cursor: pointer;
		transition:
			box-shadow var(--m3-fast-effects-dur) var(--m3-fast-effects),
			transform var(--m3-fast-spatial-dur) var(--m3-fast-spatial);
	}

	.fab:hover {
		box-shadow: var(--shadow-e4);
	}
	.fab:active {
		transform: scale(0.95);
	}

	/* ═══ Dialog — M3: 28dp, 280-560px, scrim 32% ═══ */
	.dialog-scrim {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 56px 24px;
		background: color-mix(in srgb, var(--color-scrim) 32%, transparent);
		animation: scrimIn var(--m3-fast-effects-dur) var(--m3-fast-effects) both;
	}

	.dialog-container {
		width: 100%;
		min-width: 280px;
		max-width: 560px;
		padding: 24px;
		border-radius: 28px;
		background: var(--color-surface-container-high);
		box-shadow: var(--shadow-e3);
		animation: dialogIn var(--m3-default-spatial-dur) var(--m3-default-spatial) both;
		max-height: calc(100vh - 112px);
		overflow-y: auto;
	}

	@keyframes scrimIn {
		from {
			opacity: 0;
		}
	}
	@keyframes dialogIn {
		from {
			opacity: 0;
			transform: translateY(24px) scale(0.95);
		}
	}

	.dialog-title {
		margin-bottom: 16px;
		font-size: 1.5rem;
		font-weight: 400;
		line-height: 2rem;
		color: var(--color-on-surface);
	}
	.dialog-body {
		margin-bottom: 24px;
		font-size: 0.875rem;
		font-weight: 400;
		line-height: 1.25rem;
		color: var(--color-on-surface-variant);
	}
	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		margin-top: 24px;
	}
	.error-msg {
		margin-bottom: 8px;
		font-size: 0.75rem;
		color: var(--color-error);
	}
	.divider {
		height: 1px;
		margin: 8px 0 16px;
		background: var(--color-outline-variant);
		opacity: 0.4;
	}

	/* ═══ Settings ═══ */
	.settings-section {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.settings-row {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 12px 12px;
		border-radius: 12px;
		transition: background 150ms;
	}

	.settings-row:hover {
		background: color-mix(in srgb, var(--color-on-surface) 5%, transparent);
	}

	/* Clickable settings row (open folder) */
	.settings-row-btn {
		border: none;
		background: transparent;
		cursor: pointer;
		color: inherit;
		font: inherit;
		width: 100%;
	}

	.settings-row-btn:hover {
		background: color-mix(in srgb, var(--color-primary) 8%, transparent);
	}

	/* ═══ M3 Outlined Text Field ═══ */
	.m3-field {
		position: relative;
		margin-bottom: 16px;
	}

	.m3-field input {
		width: 100%;
		height: 56px;
		padding: 0 16px;
		border: 1px solid var(--color-outline);
		border-radius: 4px;
		appearance: none;
		background: transparent;
		color: var(--color-on-surface);
		font-family: var(--font-sans);
		font-size: 1rem;
		line-height: 1.5rem;
		outline: none;
		caret-color: var(--color-primary);
		transition:
			border-color var(--m3-fast-effects-dur) var(--m3-fast-effects),
			border-width var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.m3-field input:hover {
		border-color: var(--color-on-surface);
	}
	.m3-field input:focus {
		border-color: var(--color-primary);
		border-width: 2px;
		padding: 0 15px;
	}

	.m3-field label {
		position: absolute;
		left: 12px;
		top: 50%;
		transform: translateY(-50%);
		padding: 0 4px;
		font-size: 1rem;
		color: var(--color-on-surface-variant);
		background: var(--color-surface-container-high);
		pointer-events: none;
		transition:
			top var(--m3-fast-spatial-dur) var(--m3-fast-spatial),
			font-size var(--m3-fast-effects-dur) var(--m3-fast-effects),
			color var(--m3-fast-effects-dur) var(--m3-fast-effects),
			transform var(--m3-fast-spatial-dur) var(--m3-fast-spatial);
	}

	.m3-field input:focus + label,
	.m3-field input:not(:placeholder-shown) + label {
		top: 0;
		transform: translateY(-50%);
		font-size: 0.75rem;
		color: var(--color-primary);
	}
	.m3-field input:not(:focus):not(:placeholder-shown) + label {
		color: var(--color-on-surface-variant);
	}

	/* ═══ M3 Buttons ═══ */
	.m3-btn-text {
		height: 40px;
		padding: 0 16px;
		border: none;
		border-radius: 9999px;
		background: transparent;
		color: var(--color-primary);
		font-family: var(--font-sans);
		font-size: 0.875rem;
		font-weight: 500;
		letter-spacing: 0.00625rem;
		cursor: pointer;
		transition: background var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}
	.m3-btn-text:hover {
		background: color-mix(in srgb, var(--color-primary) 8%, transparent);
	}
	.m3-btn-text:active {
		background: color-mix(in srgb, var(--color-primary) 12%, transparent);
	}

	.m3-btn-filled {
		height: 40px;
		padding: 0 24px;
		border: none;
		border-radius: 9999px;
		background: var(--color-primary);
		color: var(--color-on-primary);
		font-family: var(--font-sans);
		font-size: 0.875rem;
		font-weight: 500;
		letter-spacing: 0.00625rem;
		cursor: pointer;
		transition:
			background var(--m3-fast-effects-dur) var(--m3-fast-effects),
			box-shadow var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}
	.m3-btn-filled:hover {
		box-shadow: var(--shadow-e1);
	}
	.m3-btn-filled:active {
		box-shadow: none;
	}
	.m3-btn-error {
		background: var(--color-error);
		color: var(--color-on-error);
	}

	/* M3 Tonal button */
	.m3-btn-tonal {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		height: 40px;
		width: 100%;
		padding: 0 24px;
		border: none;
		border-radius: 9999px;
		background: var(--color-secondary-container);
		color: var(--color-on-secondary-container);
		font-family: var(--font-sans);
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		margin-bottom: 8px;
		transition: background var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.m3-btn-tonal:hover {
		background: color-mix(
			in srgb,
			var(--color-secondary-container) 88%,
			var(--color-on-secondary-container)
		);
	}

	/* ═══ Drag-and-drop overlay ═══ */
	.drop-overlay {
		position: fixed;
		inset: 0;
		z-index: 200;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--color-scrim) 60%, transparent);
		animation: scrimIn var(--m3-fast-effects-dur) var(--m3-fast-effects) both;
		pointer-events: none;
	}

	.drop-card {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		padding: 40px;
		border-radius: 28px;
		border: 2px dashed var(--color-primary);
		background: var(--color-surface-container-high);
		animation: dialogIn var(--m3-default-spatial-dur) var(--m3-default-spatial) both;
	}

	/* ═══ Import toast (snackbar) ═══ */
	.import-toast {
		position: fixed;
		bottom: 96px;
		left: 50%;
		transform: translateX(-50%);
		z-index: 150;
		padding: 12px 24px;
		border-radius: 8px;
		background: var(--color-inverse-surface);
		color: var(--color-inverse-on-surface);
		font-size: 0.875rem;
		font-weight: 500;
		box-shadow: var(--shadow-e3);
		animation: toastIn var(--m3-default-spatial-dur) var(--m3-default-spatial) both;
	}

	@keyframes toastIn {
		from {
			opacity: 0;
			transform: translateX(-50%) translateY(16px);
		}
	}
</style>
