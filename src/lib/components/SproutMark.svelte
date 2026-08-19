<script lang="ts">
  // The Sprout brand glyph (ticket 24): stem + two leaves in botanical
  // green, colored from the --brand-* tokens so it adapts to the mode.
  let { size = 18, ...rest }: { size?: number; [key: string]: unknown } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 32 32"
  fill="none"
  aria-hidden="true"
  {...rest}
>
  <path class="mark__stem" d="M16 25v-9" stroke-linecap="round" />
  <path class="mark__leaf mark__leaf--deep" d="M16 17c0-5.6-4.1-8.4-10.6-7.8-.5 5.4 2.9 8.6 10.6 7.8z" />
  <path class="mark__leaf mark__leaf--light" d="M16 14.5c0-4.5 3.2-7.2 8-6.5-.7 5.2-3.2 7.2-8 6.5z" />
</svg>

<style>
  .mark__stem {
    stroke: var(--brand-stem);
    stroke-width: 1.8;
  }

  .mark__leaf--deep {
    fill: var(--brand-leaf-deep);
  }

  .mark__leaf--light {
    fill: var(--brand-leaf-light);
  }
</style>
