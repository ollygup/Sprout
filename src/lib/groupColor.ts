/// Group color rules (ticket 225): the curated palette plus the one accepted
/// stored shape. A color is user data, never a design token — the dialog and
/// the group chrome around it stay on tokens (ADR-0028). Every palette value
/// passes the project's 3:1 non-text contrast gate against both the light and
/// dark surfaces, so one stored hex renders honestly in either theme.

export interface GroupColorSwatch {
  /** Copy key for the swatch's accessible name. */
  labelKey: string;
  /** Stored and rendered value, always lowercase `#rrggbb`. */
  hex: string;
}

/** Ten curated hues. Order is the dialog's reading order. */
export const GROUP_COLORS: readonly GroupColorSwatch[] = [
  { labelKey: "groups.color.red", hex: "#ea3b2b" },
  { labelKey: "groups.color.orange", hex: "#be6728" },
  { labelKey: "groups.color.amber", hex: "#967b29" },
  { labelKey: "groups.color.green", hex: "#1c9139" },
  { labelKey: "groups.color.teal", hex: "#248c84" },
  { labelKey: "groups.color.blue", hex: "#417ed4" },
  { labelKey: "groups.color.indigo", hex: "#6b64c9" },
  { labelKey: "groups.color.violet", hex: "#aa5cd1" },
  { labelKey: "groups.color.pink", hex: "#e03a8d" },
  { labelKey: "groups.color.slate", hex: "#737373" },
];

/** The body tint ceiling — at most 12 % of the hue, decorative only. */
export const GROUP_COLOR_TINT = 0.1;

const HEX = /^#[0-9a-fA-F]{6}$/;

/** The single accepted stored shape: `#RRGGBB`, trimmed and lowercased.
 *  Alpha, shorthand, named, rgb(), and hsl() forms are refused (never
 *  guessed at), so the backend and the field agree on one grammar. */
export function normalizeGroupColor(value: string): string | null {
  const trimmed = value.trim();
  return HEX.test(trimmed) ? trimmed.toLowerCase() : null;
}

/** The palette entry a stored color matches, if any — used to mark the
 *  current swatch. Custom hex values match nothing. */
export function groupColorSwatch(
  value: string | null | undefined
): GroupColorSwatch | null {
  if (!value) return null;
  const normalized = normalizeGroupColor(value);
  if (!normalized) return null;
  return GROUP_COLORS.find((swatch) => swatch.hex === normalized) ?? null;
}
