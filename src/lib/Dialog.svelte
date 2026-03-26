<script>
	import { fade, scale } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';

	/**
	 * M3 Basic Dialog — 28dp corners, 280-560px, scrim 32%.
	 * @type {{ open: boolean, onclose: () => void, title: string, children: import('svelte').Snippet, actions?: import('svelte').Snippet, label?: string }}
	 */
	let { open, onclose, title, children, actions, label = title } = $props();

	/** @param {KeyboardEvent} e */
	function onKeydown(e) {
		if (e.key === 'Escape') onclose();
	}
</script>

{#if open}
	<div
		class="scrim"
		onclick={onclose}
		onkeydown={onKeydown}
		role="presentation"
		transition:fade={{ duration: 150 }}
	>
		<div
			class="container"
			onclick={(e) => e.stopPropagation()}
			onkeydown={onKeydown}
			role="alertdialog"
			aria-label={label}
			tabindex="-1"
			transition:scale={{ duration: 200, start: 0.95, easing: cubicOut }}
		>
			<h2 class="title">{title}</h2>
			{@render children()}
			{#if actions}
				<div class="actions">
					{@render actions()}
				</div>
			{/if}
		</div>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 56px 24px;
		background: color-mix(in srgb, var(--color-scrim) 32%, transparent);
	}

	.container {
		width: 100%;
		min-width: 280px;
		max-width: 560px;
		padding: 24px;
		border-radius: 28px;
		background: var(--color-surface-container-high);
		box-shadow: var(--shadow-e3);
		max-height: calc(100vh - 112px);
		overflow-y: auto;
		scrollbar-width: none;
	}

	/* M3 Headline Small = 1.5rem / 400 / 2rem */
	.title {
		margin-bottom: 16px;
		font-size: 1.5rem;
		font-weight: 400;
		line-height: 2rem;
		color: var(--color-on-surface);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		margin-top: 24px;
	}
</style>
