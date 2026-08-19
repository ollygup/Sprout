// Contrast verification for the Sprout token system (ticket 11).
// Mirrors tokens.css: every text/border pair used in the UI must meet WCAG AA.
// Run: node tools/contrast-check.mjs   (expect "All checks pass")

const palettes = {
  light: {
    bg_page: "#ffffff",
    bg_surface: "#f7f6f3",
    bg_sunken: "#efeeeb",
    text: "#37352f",
    text_muted: "#6f6e69",
    accent: "#0f7b6c",
    accent_hover: "#0b6356",
    accent_tint: "rgba(15, 123, 108, 0.08)",
    on_accent: "#ffffff",
    warm_text: "#8f5d46",
    warm_tint: "rgba(159, 107, 83, 0.09)",
    danger_text: "#c43030",
    danger_tint: "rgba(224, 62, 61, 0.09)",
    info_text: "#1e6fbf",
    info_tint: "rgba(35, 131, 226, 0.09)",
    warn_text: "#825f00",
    warn_tint: "rgba(223, 171, 1, 0.12)",
    ring: "#0f7b6c",
    brand_stem: "#2f5a44",
    brand_leaf_deep: "#3f8a53",
    brand_leaf_light: "#55995f",
  },
  dark: {
    bg_page: "#191919",
    bg_surface: "#252525",
    bg_sunken: "#2e2e2e",
    text: "rgba(255, 255, 255, 0.81)",
    text_muted: "rgba(255, 255, 255, 0.5)",
    accent: "#4dab9a",
    accent_hover: "#5ab8a7",
    accent_tint: "rgba(77, 171, 154, 0.14)",
    on_accent: "#06312b",
    warm_text: "#c79c81",
    warm_tint: "rgba(181, 141, 116, 0.14)",
    danger_text: "#f26d6d",
    danger_tint: "rgba(235, 87, 87, 0.14)",
    info_text: "#6ba6f8",
    info_tint: "rgba(68, 131, 247, 0.14)",
    warn_text: "#e5a754",
    warn_tint: "rgba(229, 167, 84, 0.14)",
    ring: "#4dab9a",
    brand_stem: "#8fd6a8",
    brand_leaf_deep: "#66ab5f",
    brand_leaf_light: "#7dbf69",
  },
};

function hexToRgb(hex) {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

function parseColor(value) {
  if (Array.isArray(value)) return value;
  if (value.startsWith("#")) return hexToRgb(value);
  const m = value.match(/rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?\)/);
  if (!m) throw new Error(`cannot parse color ${value}`);
  return [Number(m[1]), Number(m[2]), Number(m[3]), m[4] === undefined ? 1 : Number(m[4])];
}

// Composite fg (possibly translucent) over a solid bg; returns a "#rrggbb" string.
function composite(fg, bg) {
  const f = parseColor(fg);
  const b = hexToRgb(bg);
  const a = f[3] ?? 1;
  const rgb = f.slice(0, 3).map((c, i) => Math.round(c * a + b[i] * (1 - a)));
  return "#" + rgb.map((c) => c.toString(16).padStart(2, "0")).join("");
}

function channel(v) {
  const c = v / 255;
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

function luminance(rgb) {
  return 0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]);
}

function contrast(fg, bg) {
  const f = luminance(parseColor(fg));
  const b = luminance(parseColor(bg));
  const [hi, lo] = f >= b ? [f, b] : [b, f];
  return (hi + 0.05) / (lo + 0.05);
}

// [fg, bg, minRatio, note]
const checks = {
  light: [
    ["text", "bg_page", 7, "primary text on page (AAA)"],
    ["text", "bg_surface", 7, "primary text on surface (AAA)"],
    ["text_muted", "bg_page", 4.5, "muted text on page"],
    ["text_muted", "bg_surface", 4.5, "muted text on surface"],
    ["accent", "bg_page", 4.5, "accent text on page"],
    ["accent", "bg_surface", 4.5, "accent text on surface"],
    ["accent", "accent_tint", 4.5, "accent badge text on tint"],
    ["on_accent", "accent", 4.5, "button text on accent"],
    ["on_accent", "accent_hover", 4.5, "button text on accent hover"],
    ["warm_text", "bg_page", 4.5, "warm text on page"],
    ["warm_text", "bg_surface", 4.5, "warm text on surface"],
    ["warm_text", "warm_tint", 4.5, "warm badge text on tint"],
    ["danger_text", "bg_page", 4.5, "danger text on page"],
    ["danger_text", "bg_surface", 4.5, "danger text on surface"],
    ["danger_text", "danger_tint", 4.5, "danger badge text on tint"],
    ["info_text", "bg_page", 4.5, "info text on page"],
    ["info_text", "bg_surface", 4.5, "info text on surface"],
    ["info_text", "info_tint", 4.5, "info badge text on tint"],
    ["warn_text", "bg_page", 4.5, "warn text on page"],
    ["warn_text", "bg_surface", 4.5, "warn text on surface"],
    ["warn_text", "warn_tint", 4.5, "warn badge text on tint"],
    ["on_accent", "danger_text", 4.5, "danger button text on danger bg"],
    ["ring", "bg_page", 3, "focus ring vs page (non-text)"],
    ["ring", "bg_surface", 3, "focus ring vs surface (non-text)"],
    ["brand_stem", "bg_surface", 3, "sprout stem vs neutral field (non-text)"],
    ["brand_leaf_deep", "bg_surface", 3, "sprout deep leaf vs neutral field (non-text)"],
    ["brand_leaf_light", "bg_surface", 3, "sprout light leaf vs neutral field (non-text)"],
  ],
  dark: [
    ["text", "bg_page", 7, "primary text on page (AAA)"],
    ["text", "bg_surface", 7, "primary text on surface (AAA)"],
    ["text_muted", "bg_page", 4.5, "muted text on page"],
    ["text_muted", "bg_surface", 4.5, "muted text on surface"],
    ["accent", "bg_page", 4.5, "accent text on page"],
    ["accent", "bg_surface", 4.5, "accent text on surface"],
    ["accent", "accent_tint", 4.5, "accent badge text on tint"],
    ["on_accent", "accent", 4.5, "button text on accent"],
    ["on_accent", "accent_hover", 4.5, "button text on accent hover"],
    ["warm_text", "bg_page", 4.5, "warm text on page"],
    ["warm_text", "bg_surface", 4.5, "warm text on surface"],
    ["warm_text", "warm_tint", 4.5, "warm badge text on tint"],
    ["danger_text", "bg_page", 4.5, "danger text on page"],
    ["danger_text", "bg_surface", 4.5, "danger text on surface"],
    ["danger_text", "danger_tint", 4.5, "danger badge text on tint"],
    ["info_text", "bg_page", 4.5, "info text on page"],
    ["info_text", "bg_surface", 4.5, "info text on surface"],
    ["info_text", "info_tint", 4.5, "info badge text on tint"],
    ["warn_text", "bg_page", 4.5, "warn text on page"],
    ["warn_text", "bg_surface", 4.5, "warn text on surface"],
    ["warn_text", "warn_tint", 4.5, "warn badge text on tint"],
    ["on_accent", "danger_text", 4.5, "danger button text on danger bg"],
    ["ring", "bg_page", 3, "focus ring vs page (non-text)"],
    ["ring", "bg_surface", 3, "focus ring vs surface (non-text)"],
    ["brand_stem", "bg_surface", 3, "sprout stem vs neutral field (non-text)"],
    ["brand_leaf_deep", "bg_surface", 3, "sprout deep leaf vs neutral field (non-text)"],
    ["brand_leaf_light", "bg_surface", 3, "sprout light leaf vs neutral field (non-text)"],
  ],
};

let failures = 0;
for (const [mode, palette] of Object.entries(palettes)) {
  console.log(`\n${mode.toUpperCase()} mode`);
  for (const [fgName, bgName, min, note] of checks[mode]) {
    // Translucent backgrounds (tints) sit on the page — composite them first.
    const bgColor = parseColor(palette[bgName])[3] === 1 ? palette[bgName] : composite(palette[bgName], palette.bg_page);
    const fg = composite(palette[fgName], bgColor);
    const bg = hexToRgb(bgColor);
    const ratio = contrast(fg, bg);
    const ok = ratio >= min;
    if (!ok) failures += 1;
    console.log(
      `${ok ? "  ok " : "FAIL "} ${fgName.padEnd(12)} on ${bgName.padEnd(12)} = ${ratio.toFixed(2)} (need ${min}) — ${note}`
    );
  }
}

if (failures > 0) {
  console.error(`\n${failures} check(s) FAILED`);
  process.exit(1);
}
console.log("\nAll checks pass — both modes meet WCAG AA (primary text AAA).");