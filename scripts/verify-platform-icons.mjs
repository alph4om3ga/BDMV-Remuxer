import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const configPath = resolve("src-tauri/tauri.conf.json");
const config = JSON.parse(readFileSync(configPath, "utf8"));
const configuredIcons = config.tauri?.bundle?.icon ?? [];

for (const icon of ["icons/icon.ico", "icons/icon.icns"]) {
  if (!configuredIcons.includes(icon)) {
    throw new Error(`${icon} is missing from the Tauri bundle icon configuration`);
  }
}

function pngDimensions(data, label) {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (data.length < 24 || !data.subarray(0, 8).equals(signature)) {
    throw new Error(`${label} is not a valid PNG image`);
  }
  return [data.readUInt32BE(16), data.readUInt32BE(20)];
}

function verifyIco(file) {
  const data = readFileSync(file);
  if (data.length < 6 || data.readUInt16LE(0) !== 0 || data.readUInt16LE(2) !== 1) {
    throw new Error(`${file} has an invalid ICO header`);
  }

  const count = data.readUInt16LE(4);
  if (count === 0 || data.length < 6 + count * 16) {
    throw new Error(`${file} has an invalid ICO directory`);
  }

  const sizes = new Set();
  for (let index = 0; index < count; index += 1) {
    const entry = 6 + index * 16;
    const width = data[entry] || 256;
    const height = data[entry + 1] || 256;
    const bitsPerPixel = data.readUInt16LE(entry + 6);
    const length = data.readUInt32LE(entry + 8);
    const offset = data.readUInt32LE(entry + 12);
    const payload = data.subarray(offset, offset + length);

    if (offset < 6 + count * 16 || length === 0 || payload.length !== length) {
      throw new Error(`${file} has an invalid image payload at directory entry ${index}`);
    }
    if (bitsPerPixel !== 32) {
      throw new Error(`${file} entry ${index} is ${bitsPerPixel}bpp; expected 32bpp`);
    }

    if (payload.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) {
      const [pngWidth, pngHeight] = pngDimensions(payload, `${file} entry ${index}`);
      if (pngWidth !== width || pngHeight !== height) {
        throw new Error(`${file} entry ${index} directory and PNG dimensions differ`);
      }
    } else {
      if (payload.length < 40 || data.readUInt32LE(offset) < 40) {
        throw new Error(`${file} entry ${index} has an invalid bitmap header`);
      }
      const bitmapWidth = data.readInt32LE(offset + 4);
      const bitmapHeight = Math.abs(data.readInt32LE(offset + 8)) / 2;
      if (bitmapWidth !== width || bitmapHeight !== height) {
        throw new Error(`${file} entry ${index} bitmap dimensions do not match its directory`);
      }
    }
    sizes.add(width);
  }

  const expected = [16, 32, 48, 64, 128, 256];
  if (expected.some((size) => !sizes.has(size))) {
    throw new Error(`${file} must include ${expected.join(", ")} pixel images`);
  }
  console.log(`Verified ${file}: ${expected.map((size) => `${size}x${size}`).join(", ")}`);
}

function verifyIcns(file) {
  const data = readFileSync(file);
  if (
    data.length < 8 ||
    data.toString("ascii", 0, 4) !== "icns" ||
    data.readUInt32BE(4) !== data.length
  ) {
    throw new Error(`${file} has an invalid ICNS header or declared length`);
  }

  const expected = new Map([
    ["icp5", 32],
    ["ic07", 128],
    ["ic08", 256],
    ["ic09", 512],
    ["ic10", 1024],
  ]);
  const found = new Set();
  let offset = 8;
  while (offset < data.length) {
    if (offset + 8 > data.length) {
      throw new Error(`${file} ends in an incomplete ICNS chunk header`);
    }
    const type = data.toString("ascii", offset, offset + 4);
    const length = data.readUInt32BE(offset + 4);
    const end = offset + length;
    if (length < 8 || end > data.length) {
      throw new Error(`${file} contains an invalid ${type} chunk`);
    }
    if (expected.has(type)) {
      const payload = data.subarray(offset + 8, end);
      const dimension = expected.get(type);
      const [width, height] = pngDimensions(payload, `${file} ${type} chunk`);
      if (width !== dimension || height !== dimension) {
        throw new Error(`${file} ${type} image is ${width}x${height}; expected ${dimension}x${dimension}`);
      }
      found.add(type);
    }
    offset = end;
  }

  const missing = [...expected.keys()].filter((type) => !found.has(type));
  if (missing.length > 0) {
    throw new Error(`${file} is missing required icon sizes: ${missing.join(", ")}`);
  }
  console.log(`Verified ${file}: 32x32, 128x128, 256x256, 512x512, 1024x1024`);
}

verifyIco(resolve("src-tauri/icons/icon.ico"));
verifyIcns(resolve("src-tauri/icons/icon.icns"));

const packagedIcnsIndex = process.argv.indexOf("--packaged-icns");
if (packagedIcnsIndex !== -1) {
  const packagedIcns = process.argv[packagedIcnsIndex + 1];
  if (!packagedIcns) {
    throw new Error("--packaged-icns requires a path to the ICNS file in the app bundle");
  }
  verifyIcns(resolve(packagedIcns));
}
