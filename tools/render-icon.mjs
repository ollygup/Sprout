// Generates the full Sprout icon set from src-tauri/icons/app-icon.svg.
// The Tauri CLI's `tauri icon` zeroes the alpha channel on resize (known
// bug), so this supersampling renderer produces every PNG directly from
// the vector geometry, then wraps them into icon.ico (PNG-embedded ICO)
// and icon.icns (PNG-embedded ICNS).
// Run: node tools/render-icon.mjs   (writes into src-tauri/icons)
// Source of truth: src-tauri/icons/app-icon.svg

import { deflateSync } from "node:zlib";
import { readFileSync, writeFileSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const iconsDir = join(root, "src-tauri", "icons");
const svg = readFileSync(join(iconsDir, "app-icon.svg"), "utf8");

// ---- minimal SVG subset parser -------------------------------------------
function attrs(tag) {
  const out = {};
  const re = /([a-zA-Z0-9_-]+)\s*=\s*"([^"]*)"/g;
  let m;
  while ((m = re.exec(tag))) out[m[1]] = m[2];
  return out;
}

function hexToRgb(h) {
  const v = h.replace("#", "");
  return [parseInt(v.slice(0, 2), 16), parseInt(v.slice(2, 4), 16), parseInt(v.slice(4, 6), 16)];
}

// Decomposes an SVG path (M, c, z) into closed loops of flattened polylines.
function flattenPath(d, flat = 0.25) {
  const loops = [];
  let loop = null;
  let x = 0,
    y = 0;
  const parts = d.match(/[Mcz]|-?[\d.]+/g) ?? [];
  let i = 0;
  const n = () => Number(parts[i++]);
  const cmd = () => (parts[i] ?? "").toUpperCase();

  function cubic(x0, y0, c1x, c1y, c2x, c2y, x1, y1, out, depth = 0) {
    const dx = x1 - x0,
      dy = y1 - y0;
    const d1 = Math.abs((c1x - x1) * dy - (c1y - y1) * dx);
    const d2 = Math.abs((c2x - x1) * dy - (c2y - y1) * dx);
    if (depth > 20 || (d1 + d2) * (d1 + d2) < flat * flat * (dx * dx + dy * dy) * 4) {
      out.push([x1, y1]);
      return;
    }
    const mx = (x0 + 3 * (c1x + c2x) + x1) / 8;
    const my = (y0 + 3 * (c1y + c2y) + y1) / 8;
    cubic(x0, y0, (x0 + c1x) / 2, (y0 + c1y) / 2, (x0 + 2 * c1x + c2x) / 4, (y0 + 2 * c1y + c2y) / 4, mx, my, out, depth + 1);
    cubic(mx, my, (x1 + 2 * c2x + c1x) / 4, (y1 + 2 * c2y + c1y) / 4, (x1 + c2x) / 2, (y1 + c2y) / 2, x1, y1, out, depth + 1);
  }

  while (i < parts.length) {
    const c = cmd();
    if (c === "M") {
      loop = [];
      loops.push(loop);
      i++;
      x = n();
      y = n();
      loop.push([x, y]);
      while (i < parts.length && /^-?[\d.]/.test(parts[i])) {
        const c1x = x + n(),
          c1y = y + n(),
          c2x = x + n(),
          c2y = y + n();
        const x1 = x + n(),
          y1 = y + n();
        cubic(x, y, c1x, c1y, c2x, c2y, x1, y1, loop);
        x = x1;
        y = y1;
      }
    } else if (c === "C") {
      i++;
      while (i < parts.length && /^-?[\d.]/.test(parts[i])) {
        const c1x = x + n(),
          c1y = y + n(),
          c2x = x + n(),
          c2y = y + n();
        const x1 = x + n(),
          y1 = y + n();
        cubic(x, y, c1x, c1y, c2x, c2y, x1, y1, loop);
        x = x1;
        y = y1;
      }
    } else if (c === "Z") {
      i++;
      loop.push(loop[0]);
      loop = null;
    } else {
      throw new Error(`unsupported path command ${c}`);
    }
  }
  return loops;
}

function inLoop(x, y, loop) {
  let inside = false;
  for (let j = 0, k = loop.length - 1; j < loop.length; k = j++) {
    const [xj, yj] = loop[j];
    const [xk, yk] = loop[k];
    if (yj > y !== yk > y && x < ((xk - xj) * (y - yj)) / (yk - yj) + xj) inside = !inside;
  }
  return inside;
}

// ---- geometry (defined in 1024-space) --------------------------------------
const baseSize = 1024;
const rect = attrs(svg.match(/<rect[^>]*>/)[0]);
const baseRx = Number(rect.rx);
const field = hexToRgb(rect.fill);

const shapes = [];
for (const p of svg.matchAll(/<path[^>]*>/g)) {
  const a = attrs(p[0]);
  if (a["fill"] && a["fill"] !== "none") {
    for (const loop of flattenPath(a.d)) {
      const xs = loop.map((pt) => pt[0]);
      const ys = loop.map((pt) => pt[1]);
      shapes.push({
        loop,
        fill: hexToRgb(a["fill"]),
        minX: Math.min(...xs),
        maxX: Math.max(...xs),
        minY: Math.min(...ys),
        maxY: Math.max(...ys),
      });
    }
  }
}

const stem = hexToRgb(svg.match(/stroke="([^"]+)"/)[1]);
const stemWidth = Number(svg.match(/stroke-width="([^"]+)"/)[1]);
const stemM = svg.match(/M([\d.]+)\s+([\d.]+)/);
const stemV = svg.match(/v(-?[\d.]+)/);
const s0 = [Number(stemM[1]), Number(stemM[2])];
const s1 = [s0[0], s0[1] + Number(stemV[1])];

// ---- rendering ---------------------------------------------------------------
function render(size) {
  const k = size / baseSize;
  const rx = baseRx * k;
  const halfW = (stemWidth * k) / 2;
  const [a0x, a0y] = [s0[0] * k, s0[1] * k];
  const [a1x, a1y] = [s1[0] * k, s1[1] * k];
  const stemLen2 = (a1x - a0x) ** 2 + (a1y - a0y) ** 2;
  const scaled = shapes.map((s) => ({
    fill: s.fill,
    minX: s.minX * k,
    maxX: s.maxX * k,
    minY: s.minY * k,
    maxY: s.maxY * k,
    loop: s.loop.map(([x, y]) => [x * k, y * k]),
  }));

  const inRoundedRect = (x, y) => {
    if (x < 0 || x > size || y < 0 || y > size) return false;
    if (x >= rx && x <= size - rx) return true;
    if (y >= rx && y <= size - rx) return true;
    const cx = x < rx ? rx : size - rx;
    const cy = y < rx ? rx : size - rx;
    return (x - cx) ** 2 + (y - cy) ** 2 <= rx * rx;
  };

  const inStem = (x, y) => {
    const dx = a1x - a0x,
      dy = a1y - a0y;
    let t = ((x - a0x) * dx + (y - a0y) * dy) / stemLen2;
    t = t < 0 ? 0 : t > 1 ? 1 : t;
    const ddx = x - (a0x + t * dx),
      ddy = y - (a0y + t * dy);
    return ddx * ddx + ddy * ddy <= halfW * halfW;
  };

  const colorAt = (x, y) => {
    if (!inRoundedRect(x, y)) return null;
    if (inStem(x, y)) return stem;
    for (const s of scaled) {
      if (x >= s.minX && x <= s.maxX && y >= s.minY && y <= s.maxY && inLoop(x, y, s.loop)) return s.fill;
    }
    return field;
  };

  const ss = size <= 64 ? 8 : size <= 256 ? 4 : 2;
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let py = 0; py < size; py++) {
    const row = py * (size * 4 + 1);
    raw[row] = 0; // filter: None
    for (let px = 0; px < size; px++) {
      let r = 0,
        g = 0,
        b = 0,
        a = 0;
      for (let sy = 0; sy < ss; sy++) {
        for (let sx = 0; sx < ss; sx++) {
          const c = colorAt(px + (sx + 0.5) / ss, py + (sy + 0.5) / ss);
          if (c) {
            r += c[0];
            g += c[1];
            b += c[2];
            a += 1;
          }
        }
      }
      const n = ss * ss;
      const o = row + 1 + px * 4;
      if (a === 0) {
        raw[o] = raw[o + 1] = raw[o + 2] = raw[o + 3] = 0;
      } else {
        raw[o] = Math.round(r / a);
        raw[o + 1] = Math.round(g / a);
        raw[o + 2] = Math.round(b / a);
        raw[o + 3] = Math.round((a / n) * 255);
      }
    }
  }
  return raw;
}

// ---- PNG encoder ---------------------------------------------------------------
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (const byte of buf) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0);
  out.write(type, 4, "ascii");
  data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}

function encodePng(raw, size) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function writePng(file, raw, size) {
  writeFileSync(join(iconsDir, file), encodePng(raw, size));
  console.log(`wrote ${file} (${size}x${size})`);
}

// ---- ICO / ICNS wrappers (PNG-embedded) --------------------------------------
function encodeIco(sizes) {
  const images = sizes.map((s) => {
    const raw = render(s);
    return { png: encodePng(raw, s), size: s };
  });
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);
  const entries = [];
  let offset = 6 + images.length * 16;
  for (const img of images) {
    const e = Buffer.alloc(16);
    e[0] = img.size >= 256 ? 0 : img.size;
    e[1] = img.size >= 256 ? 0 : img.size;
    e[2] = 0;
    e[3] = 0;
    e.writeUInt16LE(1, 4);
    e.writeUInt16LE(32, 6);
    e.writeUInt32LE(img.png.length, 8);
    e.writeUInt32LE(offset, 12);
    entries.push(e);
    offset += img.png.length;
  }
  return Buffer.concat([header, ...entries, ...images.map((i) => i.png)]);
}

function encodeIcns(sizes) {
  const images = sizes.map((s) => ({ type: s.type, png: encodePng(render(s.size), s.size) }));
  const chunks = [];
  let total = 8;
  for (const img of images) {
    const c = Buffer.alloc(8 + img.png.length);
    c.write(img.type, 0, "ascii");
    c.writeUInt32BE(8 + img.png.length, 4);
    img.png.copy(c, 8);
    chunks.push(c);
    total += c.length;
  }
  const head = Buffer.alloc(8);
  head.write("icns", 0, "ascii");
  head.writeUInt32BE(total, 4);
  return Buffer.concat([head, ...chunks]);
}

// ---- emit the set --------------------------------------------------------------
// The old CLI-generated files (ios/, android/) are stale garbage from a
// broken rasterizer — this script owns the set, so drop them.
for (const dir of ["ios", "android"]) rmSync(join(iconsDir, dir), { recursive: true, force: true });

writePng("32x32.png", render(32), 32);
writePng("64x64.png", render(64), 64);
writePng("128x128.png", render(128), 128);
writePng("128x128@2x.png", render(256), 256);
writePng("icon.png", render(512), 512);
writePng("app-icon.png", render(1024), 1024);
writePng("StoreLogo.png", render(50), 50);
for (const s of [30, 44, 71, 89, 107, 142, 150, 284, 310]) writePng(`Square${s}x${s}Logo.png`, render(s), s);

writeFileSync(
  join(iconsDir, "icon.ico"),
  encodeIco([16, 24, 32, 48, 64, 128, 256])
);
console.log("wrote icon.ico (16,24,32,48,64,128,256)");

writeFileSync(
  join(iconsDir, "icon.icns"),
  encodeIcns([
    { type: "icp4", size: 16 },
    { type: "icp5", size: 32 },
    { type: "icp6", size: 64 },
    { type: "ic07", size: 128 },
    { type: "ic08", size: 256 },
    { type: "ic09", size: 512 },
    { type: "ic10", size: 1024 },
  ])
);
console.log("wrote icon.icns (16..1024)");