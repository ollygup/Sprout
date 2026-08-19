<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLSelectAttributes } from "svelte/elements";
  import Icon from "./Icon.svelte";

  /** Shared select (ticket 45): one treatment for every dropdown in the app.
   *  Tokens supply the background/color (explicit in both themes), a custom
   *  chevron replaces the OS arrow, and the focus ring matches the text
   *  inputs. Variants: default (body, full size), small (body, tighter),
   *  compact (mono, for env wiring action rows). */
  type Rest = Omit<HTMLSelectAttributes, "value" | "onchange" | "class" | "children">;

  let {
    id,
    value,
    onchange,
    variant = "default",
    class: klass,
    children,
    ...rest
  }: {
    id?: string;
    value: string;
    onchange: (v: string) => void;
    variant?: "default" | "small" | "compact";
    class?: string;
    children: Snippet;
  } & Rest = $props();
</script>

<div
  class="select {variant === "small" ? "select--small" : ""}{variant === "compact" ? " select--compact" : ""} {klass}"
>
  <select
    {id}
    class="select__control"
    {value}
    onchange={(e) => onchange((e.target as HTMLSelectElement).value)}
    {...rest}
  >
    {@render children()}
  </select>
  <span class="select__chevron" aria-hidden="true">
    <Icon name="chevron-down" size={14} />
  </span>
</div>

<style>
  .select {
    position: relative;
    display: inline-flex;
    min-width: 0;
  }

  .select__control {
    appearance: none;
    width: 100%;
    min-width: 0;
    font-family: var(--font-body);
    font-size: var(--text-base);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 8px 30px 8px 10px;
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .select__control:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .select--small .select__control {
    font-size: var(--text-sm);
    padding: 6px 26px 6px 8px;
  }

  .select--compact .select__control {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    border-radius: var(--radius-sm);
    padding: 6px 24px 6px 8px;
  }

  .select__chevron {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    display: inline-flex;
    color: var(--text-muted);
    pointer-events: none;
  }

  .select--compact .select__chevron {
    right: 6px;
  }
</style>
