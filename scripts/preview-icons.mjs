import { mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = resolve(projectRoot, "src-tauri/icons/icon.svg");
const outputPath = resolve(projectRoot, "generated-artifacts/icon-preview.png");
const samples = [
  { size: 16, label: "16 px — Windows ICO / taskbar", zoom: 12 },
  { size: 32, label: "32 px — ICO + configured PNG", zoom: 6 },
  { size: 128, label: "128 px — configured PNG", zoom: 2 },
  { size: 256, label: "256 px — 128 px @2x PNG", zoom: 1 },
];

const width = 720;
const height = 870;
const cardWidth = 328;
const cardHeight = 316;
const positions = [
  { left: 24, top: 64 },
  { left: 368, top: 64 },
  { left: 24, top: 392 },
  { left: 368, top: 392 },
];

const escapeXml = (text) =>
  text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");

const cardMarkup = samples
  .map(({ size, label, zoom }, index) => {
    const { left, top } = positions[index];
    const displaySize = size * zoom;
    const imageTop = top + 60 + (256 - displaySize) / 2;
    return `
      <rect x="${left}" y="${top}" width="${cardWidth}" height="${cardHeight}" rx="18" fill="#ffffff" stroke="#cbd5e1" stroke-width="2"/>
      <text x="${left + 18}" y="${top + 30}" fill="#14243a" font-size="17" font-weight="700">${escapeXml(label)}</text>
      <text x="${left + 18}" y="${top + 51}" fill="#526174" font-size="12">Pixel-perfect ${zoom}× zoom</text>`;
  })
  .join("");

const page = Buffer.from(`
  <svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
    <rect width="100%" height="100%" fill="#eef2f7"/>
    <text x="24" y="32" fill="#14243a" font-size="23" font-weight="700">BDMV Remuxer icon size review</text>
    <text x="24" y="52" fill="#526174" font-size="13">Rendered from src-tauri/icons/icon.svg at the listed pixel dimensions</text>
    ${cardMarkup}
    <rect x="24" y="724" width="672" height="116" rx="18" fill="#14243a"/>
    <text x="42" y="751" fill="#ffffff" font-size="15" font-weight="700">Native-size taskbar check (1:1 pixels)</text>
    <text x="42" y="780" fill="#d8e2ee" font-size="12">16 px</text>
    <text x="104" y="780" fill="#d8e2ee" font-size="12">32 px</text>
    <text x="176" y="780" fill="#d8e2ee" font-size="12">At 16 px and 32 px, confirm the play triangle stays distinct</text>
    <text x="176" y="801" fill="#d8e2ee" font-size="12">and the disc rings and arrow remain readable.</text>
  </svg>
`);

const composites = await Promise.all(
  samples.map(async ({ size, zoom }, index) => {
    const displaySize = size * zoom;
    const { left, top } = positions[index];
    const imageLeft = left + (cardWidth - displaySize) / 2;
    const imageTop = top + 60 + (256 - displaySize) / 2;
    return {
      input: await sharp(sourcePath)
        .resize(size, size, { fit: "contain" })
        .resize(displaySize, displaySize, { kernel: "nearest" })
        .png()
        .toBuffer(),
      left: imageLeft,
      top: imageTop,
    };
  }),
);

const nativeIcons = await Promise.all(
  samples.slice(0, 2).map(async ({ size }, index) => ({
    input: await sharp(sourcePath).resize(size, size, { fit: "contain" }).png().toBuffer(),
    left: index === 0 ? 70 : 124,
    top: 787,
  })),
);

await mkdir(dirname(outputPath), { recursive: true });
await sharp(page)
  .composite([...composites, ...nativeIcons])
  .png()
  .toFile(outputPath);

console.log(`Wrote ${outputPath}`);
