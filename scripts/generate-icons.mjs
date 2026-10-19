import { execFileSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = resolve(projectRoot, "src-tauri/icons/icon.svg");
const tauriConfigPath = resolve(projectRoot, "src-tauri/tauri.conf.json");
const tauriConfigDirectory = dirname(tauriConfigPath);
const tauriCliPath = resolve(projectRoot, "node_modules/@tauri-apps/cli/tauri.js");
const checkOnly = process.argv.includes("--check");
const tempDirectory = await mkdtemp(join(tmpdir(), "bdmv-remuxer-icons-"));
const rasterPath = join(tempDirectory, "icon.png");
const generatedIconsPath = join(tempDirectory, "generated");
const config = JSON.parse(await readFile(tauriConfigPath, "utf8"));
const configuredIcons = config.tauri?.bundle?.icon;

try {
  if (!Array.isArray(configuredIcons) || configuredIcons.length === 0) {
    throw new Error(`No bundle icons are configured in ${tauriConfigPath}`);
  }

  await mkdir(generatedIconsPath);
  await sharp(sourcePath)
    .resize(1024, 1024, { fit: "contain" })
    .png()
    .toFile(rasterPath);

  execFileSync(
    process.execPath,
    [tauriCliPath, "icon", rasterPath, "--output", generatedIconsPath],
    { cwd: projectRoot, stdio: "inherit" },
  );

  const icnsPath = join(generatedIconsPath, "icon.icns");
  const icns = await readFile(icnsPath);
  let has32PixelChunk = false;
  const icnsChunks = [];
  for (let offset = 8; offset < icns.length; ) {
    if (offset + 8 > icns.length) {
      throw new Error(`Generated ICNS file has an incomplete chunk header: ${icnsPath}`);
    }
    const chunkType = icns.toString("ascii", offset, offset + 4);
    const chunkLength = icns.readUInt32BE(offset + 4);
    if (chunkLength < 8 || offset + chunkLength > icns.length) {
      throw new Error(`Generated ICNS file has an invalid ${chunkType} chunk: ${icnsPath}`);
    }
    if (chunkType === "icp5") has32PixelChunk = true;
    icnsChunks.push(icns.subarray(offset, offset + chunkLength));
    offset += chunkLength;
  }
  if (!has32PixelChunk) {
    const image32 = await sharp(rasterPath).resize(32, 32).png().toBuffer();
    const chunk = Buffer.alloc(8 + image32.length);
    chunk.write("icp5", 0, 4, "ascii");
    chunk.writeUInt32BE(chunk.length, 4);
    image32.copy(chunk, 8);
    icnsChunks.push(chunk);
  }
  icnsChunks.sort(
    (left, right) =>
      Buffer.compare(left.subarray(0, 4), right.subarray(0, 4)) || Buffer.compare(left, right),
  );
  const normalizedIcnsLength = 8 + icnsChunks.reduce((total, chunk) => total + chunk.length, 0);
  const normalizedIcns = Buffer.alloc(normalizedIcnsLength);
  normalizedIcns.write("icns", 0, 4, "ascii");
  normalizedIcns.writeUInt32BE(normalizedIcnsLength, 4);
  let icnsOffset = 8;
  for (const chunk of icnsChunks) {
    chunk.copy(normalizedIcns, icnsOffset);
    icnsOffset += chunk.length;
  }
  await writeFile(icnsPath, normalizedIcns);

  const icoSizes = [16, 32, 48, 64, 128, 256];
  const icoImages = await Promise.all(
    icoSizes.map((size) => sharp(rasterPath).resize(size, size).png().toBuffer()),
  );
  const directoryLength = 6 + icoSizes.length * 16;
  const icoLength = directoryLength + icoImages.reduce((total, image) => total + image.length, 0);
  const ico = Buffer.alloc(icoLength);
  ico.writeUInt16LE(0, 0);
  ico.writeUInt16LE(1, 2);
  ico.writeUInt16LE(icoSizes.length, 4);

  let imageOffset = directoryLength;
  for (let index = 0; index < icoSizes.length; index += 1) {
    const size = icoSizes[index];
    const image = icoImages[index];
    const entryOffset = 6 + index * 16;
    ico[entryOffset] = size === 256 ? 0 : size;
    ico[entryOffset + 1] = size === 256 ? 0 : size;
    ico.writeUInt16LE(1, entryOffset + 4);
    ico.writeUInt16LE(32, entryOffset + 6);
    ico.writeUInt32LE(image.length, entryOffset + 8);
    ico.writeUInt32LE(imageOffset, entryOffset + 12);
    image.copy(ico, imageOffset);
    imageOffset += image.length;
  }
  await writeFile(join(generatedIconsPath, "icon.ico"), ico);

  for (const iconPath of configuredIcons) {
    const generatedPath = join(generatedIconsPath, iconPath.split(/[\\/]/).at(-1));
    const destinationPath = resolve(tauriConfigDirectory, iconPath);
    if (checkOnly) {
      let currentIcon;
      try {
        currentIcon = await readFile(destinationPath);
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
      }

      const generatedIcon = await readFile(generatedPath);
      if (!currentIcon || !currentIcon.equals(generatedIcon)) {
        console.error(`Stale or missing Tauri icon: ${iconPath}`);
        process.exitCode = 1;
      } else {
        console.log(`Fresh ${iconPath}`);
      }
    } else {
      await mkdir(dirname(destinationPath), { recursive: true });
      await copyFile(generatedPath, destinationPath);
      console.log(`Updated ${iconPath}`);
    }
  }

  if (checkOnly && process.exitCode === 1) {
    throw new Error("Tauri icons are stale. Run `npm run icons:generate` to update them.");
  }
} finally {
  await rm(tempDirectory, { recursive: true, force: true });
}
