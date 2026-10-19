import { mkdir, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tauriConfigPath = resolve(projectRoot, "src-tauri/tauri.conf.json");
const tauriConfigDirectory = dirname(tauriConfigPath);
const sourcePath = resolve(projectRoot, "src-tauri/icons/icon.svg");
const outputPath = resolve(projectRoot, "generated-artifacts/icon-preview.png");
const config = JSON.parse(await readFile(tauriConfigPath, "utf8"));
const configuredIcons = config.tauri?.bundle?.icon;

const icnsImageSizes = new Map([
  ["ic04", 16],
  ["ic05", 32],
  ["ic06", 64],
  ["ic07", 128],
  ["ic08", 256],
  ["ic09", 512],
  ["ic10", 1024],
  ["ic11", 32],
  ["ic12", 64],
  ["ic13", 256],
  ["ic14", 512],
  ["icp4", 16],
  ["icp5", 32],
  ["icp6", 64],
  ["is32", 16],
  ["il32", 32],
  ["ih32", 48],
  ["it32", 128],
  ["icm4", 16],
  ["icm5", 32],
  ["icm6", 64],
]);
const icnsNonImageChunks = new Set(["l8mk", "s8mk", "t8mk", "TOC ", "name", "info"]);

function previewSupportError(iconPath, detail) {
  return new Error(
    `Cannot preview configured Tauri icon "${iconPath}": ${detail}. ` +
      "Update scripts/preview-icons.mjs to support this bundle icon before changing tauri.conf.json.",
  );
}

function addConfiguredSample(samples, size, iconPath) {
  if (!Number.isInteger(size) || size < 1) {
    throw previewSupportError(iconPath, `invalid image size ${size}`);
  }
  const existing = samples.get(size) ?? { size, iconPaths: [] };
  if (!existing.iconPaths.includes(iconPath)) existing.iconPaths.push(iconPath);
  samples.set(size, existing);
}

function getIcoSizes(iconPath, contents) {
  if (
    contents.length < 6 ||
    contents.readUInt16LE(0) !== 0 ||
    contents.readUInt16LE(2) !== 1
  ) {
    throw previewSupportError(iconPath, "the ICO header is invalid");
  }

  const count = contents.readUInt16LE(4);
  if (count === 0 || contents.length < 6 + count * 16) {
    throw previewSupportError(iconPath, "the ICO image directory is invalid");
  }

  return Array.from({ length: count }, (_, index) => {
    const offset = 6 + index * 16;
    const width = contents[offset] || 256;
    const height = contents[offset + 1] || 256;
    if (width !== height) {
      throw previewSupportError(iconPath, `non-square ICO entry ${width}×${height}`);
    }
    return width;
  });
}

function getIcnsSizes(iconPath, contents) {
  if (contents.length < 8 || contents.toString("ascii", 0, 4) !== "icns") {
    throw previewSupportError(iconPath, "the ICNS header is invalid");
  }

  const declaredLength = contents.readUInt32BE(4);
  if (declaredLength !== contents.length) {
    throw previewSupportError(iconPath, "the ICNS file length does not match its header");
  }

  const sizes = [];
  for (let offset = 8; offset < contents.length; ) {
    if (offset + 8 > contents.length) {
      throw previewSupportError(iconPath, "an ICNS chunk header is incomplete");
    }
    const chunkType = contents.toString("ascii", offset, offset + 4);
    const chunkLength = contents.readUInt32BE(offset + 4);
    if (chunkLength < 8 || offset + chunkLength > contents.length) {
      throw previewSupportError(iconPath, `the ICNS ${chunkType} chunk is invalid`);
    }

    const size = icnsImageSizes.get(chunkType);
    if (size) {
      sizes.push(size);
    } else if (!icnsNonImageChunks.has(chunkType)) {
      throw previewSupportError(iconPath, `the ICNS ${chunkType} chunk has an unknown size`);
    }
    offset += chunkLength;
  }

  if (sizes.length === 0) {
    throw previewSupportError(iconPath, "no supported image sizes were found in the ICNS file");
  }
  return sizes;
}

if (!Array.isArray(configuredIcons) || configuredIcons.length === 0) {
  throw new Error(`No bundle icons are configured in ${tauriConfigPath}`);
}

const samplesBySize = new Map();
for (const iconPath of configuredIcons) {
  const iconFilePath = resolve(tauriConfigDirectory, iconPath);
  const extension = iconPath.split(".").at(-1).toLowerCase();

  if (extension === "png") {
    let metadata;
    try {
      metadata = await sharp(iconFilePath).metadata();
    } catch (error) {
      throw previewSupportError(iconPath, `the PNG cannot be read (${error.message})`);
    }
    if (metadata.format !== "png") {
      throw previewSupportError(iconPath, `the configured PNG contains ${metadata.format ?? "unknown data"}`);
    }
    if (metadata.width !== metadata.height) {
      throw previewSupportError(iconPath, `non-square PNG dimensions ${metadata.width}×${metadata.height}`);
    }
    addConfiguredSample(samplesBySize, metadata.width, iconPath);
  } else if (extension === "ico" || extension === "icns") {
    let contents;
    try {
      contents = await readFile(iconFilePath);
    } catch (error) {
      throw previewSupportError(iconPath, `the file cannot be read (${error.message})`);
    }
    const sizes =
      extension === "ico"
        ? getIcoSizes(iconPath, contents)
        : getIcnsSizes(iconPath, contents);
    for (const size of sizes) addConfiguredSample(samplesBySize, size, iconPath);
  } else {
    throw previewSupportError(iconPath, `the .${extension} format is not supported`);
  }
}

const samples = [...samplesBySize.values()].sort((left, right) => left.size - right.size);
const nativeSamples = samples.filter(({ size }) => size === 16 || size === 32);
const width = 720;
const margin = 24;
const cardWidth = 328;
const cardHeight = 316;
const columnGap = 16;
const rowGap = 12;
const gridTop = 64;
const columns = 2;
const rows = Math.ceil(samples.length / columns);
const gridBottom = gridTop + rows * cardHeight + (rows - 1) * rowGap;
const nativeFooterTop = gridBottom + 16;
const nativeFooterHeight = nativeSamples.length > 0 ? 116 : 0;
const height = gridBottom + (nativeFooterHeight ? 16 + nativeFooterHeight : 0) + margin;
const positions = samples.map((_, index) => ({
  left: margin + (index % columns) * (cardWidth + columnGap),
  top: gridTop + Math.floor(index / columns) * (cardHeight + rowGap),
}));
const previewLimit = 256;
const zoomFor = (size) =>
  size <= previewLimit ? Math.min(12, Math.max(1, Math.floor(previewLimit / size))) : previewLimit / size;

const escapeXml = (text) =>
  text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");

const cardMarkup = samples
  .map(({ size, iconPaths }, index) => {
    const { left, top } = positions[index];
    const zoom = zoomFor(size);
    const displaySize = Math.min(previewLimit, size * zoom);
    const scaleLabel = `${Number(zoom.toFixed(2))}× preview scale`;
    return `
      <rect x="${left}" y="${top}" width="${cardWidth}" height="${cardHeight}" rx="18" fill="#ffffff" stroke="#cbd5e1" stroke-width="2"/>
      <text x="${left + 18}" y="${top + 30}" fill="#14243a" font-size="17" font-weight="700">${size} px</text>
      <text x="${left + 18}" y="${top + 51}" fill="#526174" font-size="12">${iconPaths.length} configured bundle icon${iconPaths.length === 1 ? "" : "s"} · ${escapeXml(scaleLabel)}</text>`;
  })
  .join("");
const nativeFooter = nativeSamples.length
  ? `
    <rect x="24" y="${nativeFooterTop}" width="672" height="${nativeFooterHeight}" rx="18" fill="#14243a"/>
    <text x="42" y="${nativeFooterTop + 27}" fill="#ffffff" font-size="15" font-weight="700">Native-size icon check (1:1 pixels)</text>
    ${nativeSamples
      .map(({ size }, index) => {
        const cellLeft = 42 + index * 48;
        return `
          <text x="${cellLeft + 24}" y="${nativeFooterTop + 102}" fill="#d8e2ee" font-size="12" text-anchor="middle">${size} px</text>`;
      })
      .join("")}
    <text x="${42 + nativeSamples.length * 48 + 8}" y="${nativeFooterTop + 60}" fill="#d8e2ee" font-size="12">At the smallest configured sizes, confirm the play triangle stays distinct</text>
    <text x="${42 + nativeSamples.length * 48 + 8}" y="${nativeFooterTop + 81}" fill="#d8e2ee" font-size="12">and the disc rings and arrow remain readable.</text>`
  : "";

const page = Buffer.from(`
  <svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
    <rect width="100%" height="100%" fill="#eef2f7"/>
    <text x="24" y="32" fill="#14243a" font-size="23" font-weight="700">BDMV Remuxer icon size review</text>
    <text x="24" y="52" fill="#526174" font-size="13">Every configured Tauri bundle icon size, rendered from src-tauri/icons/icon.svg</text>
    ${cardMarkup}
    ${nativeFooter}
  </svg>
`);

const composites = await Promise.all(
  samples.map(async ({ size }, index) => {
    const { left, top } = positions[index];
    const zoom = zoomFor(size);
    const displaySize = Math.min(previewLimit, size * zoom);
    const renderedIcon = await sharp(sourcePath)
      .resize(size, size, { fit: "contain" })
      .png()
      .toBuffer();
    const input = await sharp(renderedIcon)
      .resize(displaySize, displaySize, {
        kernel: displaySize > size ? "nearest" : "lanczos3",
      })
      .png()
      .toBuffer();
    return {
      input,
      left: left + (cardWidth - displaySize) / 2,
      top: top + 60 + (previewLimit - displaySize) / 2,
    };
  }),
);
const nativeIcons = await Promise.all(
  nativeSamples.map(async ({ size }, index) => {
    const cellLeft = 42 + index * 48;
    return {
      input: await sharp(sourcePath).resize(size, size, { fit: "contain" }).png().toBuffer(),
      left: cellLeft + (48 - size) / 2,
      top: nativeFooterTop + 45,
    };
  }),
);

await mkdir(dirname(outputPath), { recursive: true });
await sharp(page).composite([...composites, ...nativeIcons]).png().toFile(outputPath);

console.log(
  `Wrote ${outputPath} with configured bundle sizes: ${samples.map(({ size }) => size).join(", ")} px`,
);
