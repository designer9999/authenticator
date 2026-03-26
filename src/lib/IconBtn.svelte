<script>
	/**
	 * M3 Standard Icon Button — 40px circle, 8% hover, FILL 0→1 on press.
	 * @type {{ icon: string, label: string, onclick?: (e: MouseEvent) => void, class?: string }}
	 */
	let { icon, label, onclick, class: klass = '' } = $props();
</script>

<button class="m3-icon-btn {klass}" {onclick} aria-label={label} title={label}>
	<span class="material-symbols-outlined">{icon}</span>
</button>

<style>
	.m3-icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 40px;
		height: 40px;
		flex-shrink: 0;
		border: none;
		border-radius: 9999px;
		background: transparent;
		color: var(--color-on-surface-variant);
		cursor: pointer;
		transition: background var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.m3-icon-btn .material-symbols-outlined {
		font-size: 24px;
		font-variation-settings:
			'FILL' 0,
			'wght' 400,
			'GRAD' -25,
			'opsz' 24;
		transition: font-variation-settings var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.m3-icon-btn:hover {
		background: color-mix(in srgb, var(--color-on-surface) 8%, transparent);
	}

	.m3-icon-btn:active .material-symbols-outlined {
		font-variation-settings:
			'FILL' 1,
			'wght' 400,
			'GRAD' -25,
			'opsz' 24;
	}
</style>
