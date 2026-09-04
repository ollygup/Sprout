<script lang="ts">
  let {
    name,
    size = 16,
    label,
    ...rest
  }: {
    name: string;
    size?: number;
    label?: string;
    [key: string]: unknown;
  } = $props();
  const paths: Record<string, string> = {
    search:
      '<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    pencil: '<path d="M17 3a2.8 2.8 0 0 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/>',
    trash:
      '<path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><path d="M10 11v6M14 11v6"/>',
    x: '<path d="M18 6 6 18M6 6l12 12"/>',
    copy: '<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>',
    download:
      '<path d="M12 3v12"/><path d="m7 10 5 5 5-5"/><path d="M5 21h14"/>',
    folder:
      '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/><path d="M3 7v11a2 2 0 0 0 2 2h14"/>',
    seedling:
      '<path d="M12 20v-7"/><path d="M12 13c0-4.2-3.1-6.3-8-5.8C3.4 10.6 6 13 12 13z"/><path d="M12 11c0-3.4 2.4-5.4 6-4.9-.5 3.9-2.4 5.4-6 4.9z"/>',
    dots: '<path d="M5 12h.01M12 12h.01M19 12h.01"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 8h.01"/><path d="M12 12v4"/>',
    monitor:
      '<rect x="3" y="4.5" width="18" height="13" rx="2"/><path d="M9 21h6"/>',
    play: '<path d="M7 4.5v15l13-7.5z"/>',
    stop: '<rect x="6" y="6" width="12" height="12" rx="1.5"/>',
    chevron: '<path d="m6 9 6 6 6-6"/>',
    "chevron-up": '<path d="m6 15 6-6 6 6"/>',
    "chevron-down": '<path d="m6 9 6 6 6-6"/>',
    caret:
      '<path d="m9 6.2 8.4 5a.94.94 0 0 1 0 1.6l-8.4 5A.8.8 0 0 1 8 17.1V6.9a.8.8 0 0 1 1-.7Z" fill="currentColor" stroke="none"/>',
    refresh: '<path d="M19 12a7 7 0 1 1-2.05-4.95"/><path d="M19 4v4h-4"/>',
    rocket:
      '<path d="M5 15c-1.5 1.3-2 5-2 5s3.7-.5 5-2"/><path d="M9 15 6 12c1.5-5 6-9 12-9 0 6-4 10.5-9 12z"/><path d="M15 9h.01"/>',
    terminal:
      '<path d="m5 7 5 5-5 5"/><path d="M12 17h7"/>',
    check: '<path d="m4.5 12.5 5 5L19.5 7"/>',
    "dock-left":
      '<rect x="6" y="4" width="15" height="16" rx="2"/><rect x="3" y="4" width="3" height="16" rx="1.5"/>',
    "dock-right":
      '<rect x="3" y="4" width="15" height="16" rx="2"/><rect x="18" y="4" width="3" height="16" rx="1.5"/>',
    undock:
      '<rect x="6" y="4" width="14" height="16" rx="2"/><path d="M4 9h16"/>',
    "chevron-left": '<path d="m15 6-6 6 6 6"/>',
    "chevron-right": '<path d="m9 6 6 6-6 6"/>',
    warn:
      '<path d="M12 4 2.8 20h18.4L12 4Z"/><path d="M12 10.5v4.5"/><path d="M12 18h.01"/>',
    gear:
      '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
    // Ticket 118 — content-gated note glyph (pattern 14)
    note: '<path d="M14 2H7a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M9 13h6"/><path d="M9 17h6"/>',
    // Ticket 125 — external open (companion)
    external: '<path d="M14 4h6v6"/><path d="M10 14 20 4"/><path d="M20 14v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h6"/>',
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.7"
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
  {...rest}
>
  {@html paths[name] ?? ""}
</svg>
