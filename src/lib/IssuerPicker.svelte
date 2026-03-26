<script>
	import { getAllBrands, getBrand } from '$lib/brands.js';

	/** @type {{ onselect: (issuer: string) => void, selected?: string }} */
	let { onselect, selected = '' } = $props();

	const brands = getAllBrands();

	/** @type {HTMLDivElement|undefined} */
	let rowEl = $state();

	/** @param {WheelEvent} e */
	function onWheel(e) {
		if (!rowEl) return;
		e.preventDefault();
		rowEl.scrollLeft += e.deltaY;
	}
</script>

<div
	class="chip-row"
	bind:this={rowEl}
	onwheel={onWheel}
	role="listbox"
	aria-label="Choose issuer"
	tabindex="0"
>
	{#each brands as name (name)}
		{@const brand = getBrand(name)}
		{@const active = selected.toLowerCase() === name.toLowerCase()}
		<button class="chip" class:active onclick={() => onselect(name)} title={name}>
			<span class="chip-icon" class:active>
				{#if active}
					<span class="material-symbols-outlined icon-filled chip-check">check</span>
				{:else if brand.svg}
					<!-- eslint-disable-next-line svelte/no-at-html-tags -->
					{@html brand.svg}
				{:else}
					{brand.letter}
				{/if}
			</span>
			{name}
		</button>
	{/each}
</div>

<style>
	.chip-row {
		display: flex;
		gap: 8px;
		padding: 12px 0;
		overflow-x: auto;
		scroll-behavior: smooth;
		scroll-snap-type: x proximity;
		scrollbar-width: none;
		-webkit-overflow-scrolling: touch;
	}

	.chip-row::-webkit-scrollbar {
		display: none;
	}

	/* M3 Filter Chip: 32dp height, 8dp radius, outline border */
	.chip {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 32px;
		padding: 0 16px 0 8px;
		flex-shrink: 0;
		border: 1px solid var(--color-outline);
		border-radius: 8px;
		background: transparent;
		color: var(--color-on-surface-variant);
		font-family: var(--font-sans);
		font-size: 0.875rem;
		font-weight: 500;
		white-space: nowrap;
		cursor: pointer;
		scroll-snap-align: start;
		transition:
			background var(--m3-fast-effects-dur) var(--m3-fast-effects),
			border-color var(--m3-fast-effects-dur) var(--m3-fast-effects),
			color var(--m3-fast-effects-dur) var(--m3-fast-effects);
	}

	.chip:hover {
		background: color-mix(in srgb, var(--color-on-surface) 8%, transparent);
	}

	.chip.active {
		background: var(--color-secondary-container);
		color: var(--color-on-secondary-container);
		border-color: transparent;
	}

	.chip.active:hover {
		background: color-mix(
			in srgb,
			var(--color-secondary-container) 92%,
			var(--color-on-secondary-container)
		);
	}

	.chip-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 18px;
		border-radius: 9999px;
		overflow: hidden;
		color: var(--color-primary);
	}

	.chip-icon.active {
		color: var(--color-on-secondary-container);
	}

	.chip-icon :global(svg) {
		width: 18px;
		height: 18px;
	}

	.chip-check {
		font-size: 18px;
	}
</style>
