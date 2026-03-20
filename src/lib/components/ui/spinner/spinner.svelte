<script lang="ts">
	import { cn } from "$lib/utils.js";
	import Loader2Icon from '@lucide/svelte/icons/loader-2';
	import type { IconProps } from "@lucide/svelte";

	let {
		class: className,
		role = "status",
		color,
		stroke,
		"aria-label": ariaLabel = "Loading",
		...restProps
	}: IconProps = $props();
</script>

<Loader2Icon {role} {color} {stroke} aria-label={ariaLabel} class={cn("size-4 animate-spin", className)} {...restProps} />
