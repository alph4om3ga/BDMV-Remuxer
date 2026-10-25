import { readFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const defaultSourcePath = resolve(projectRoot, "src-tauri/icons/icon.svg");
const playTriangle = [
  [453, 460],
  [453, 564],
  [563, 512],
];
const ringThresholds = {
  16: [
    { radius: 290, minimum: 0.06, label: "outer disc ring" },
    { radius: 248, minimum: 0.065, label: "bright disc ring" },
    { radius: 196, minimum: 0.025, label: "inner disc edge" },
    { radius: 128, minimum: 0.2, label: "center opening" },
  ],
  32: [
    { radius: 290, minimum: 0.08, label: "outer disc ring" },
    { radius: 248, minimum: 0.09, label: "bright disc ring" },
    { radius: 196, minimum: 0.05, label: "inner disc edge" },
    { radius: 128, minimum: 0.3, label: "center opening" },
  ],
  48: [
    { radius: 290, minimum: 0.08, label: "outer disc ring" },
    { radius: 248, minimum: 0.06, label: "bright disc ring" },
    { radius: 196, minimum: 0.05, label: "inner disc edge" },
    { radius: 128, minimum: 0.3, label: "center opening" },
  ],
  64: [
    { radius: 290, minimum: 0.08, label: "outer disc ring" },
    { radius: 248, minimum: 0.08, label: "bright disc ring" },
    { radius: 196, minimum: 0.05, label: "inner disc edge" },
    { radius: 128, minimum: 0.3, label: "center opening" },
  ],
};

const iconSizes = [16, 32, 48, 64];
const minimumPlayPixels = { 16: 2, 32: 2, 48: 4, 64: 6 };
const minimumArrowPixels = { 16: 8, 32: 20, 48: 45, 64: 80 };
const ringSampleOffsets = { 16: 24, 32: 24, 48: 12, 64: 8 };

function luminance([red, green, blue]) {
  const linear = [red, green, blue].map((channel) => {
    const srgb = channel / 255;
    return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)];
}

function pointInTriangle(point, [first, second, third]) {
  const side = (a, b, p) =>
    (p[0] - b[0]) * (a[1] - b[1]) - (a[0] - b[0]) * (p[1] - b[1]);
  const firstSide = side(first, second, point) < 0;
  const secondSide = side(second, third, point) < 0;
  const thirdSide = side(third, first, point) < 0;
  return firstSide === secondSide && secondSide === thirdSide;
}

function analyzePlaySymbol(data, size) {
  const pixelAt = (x, y) => {
    const offset = (y * size + x) * 3;
    return luminance(data.subarray(offset, offset + 3));
  };
  const background = [];
  const foreground = [];

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const sourceX = ((x + 0.5) * 1024) / size;
      const sourceY = ((y + 0.5) * 1024) / size;
      const radius = Math.hypot(sourceX - 512, sourceY - 512);
      const value = pixelAt(x, y);

      if (radius >= 60 && radius <= 120 && sourceX < 445) {
        background.push(value);
      }

      if (
        radius > 40 &&
        pointInTriangle([sourceX, sourceY], playTriangle)
      ) {
        foreground.push(value);
      }
    }
  }

  if (background.length === 0 || foreground.length === 0) {
    return { visiblePixels: 0, contrastRatio: 1 };
  }

  const backgroundLuminance = median(background);
  const contrastRatios = foreground.map(
    (value) => (Math.max(value, backgroundLuminance) + 0.05) /
      (Math.min(value, backgroundLuminance) + 0.05),
  );
  return {
    visiblePixels: contrastRatios.filter((ratio) => ratio >= 3).length,
    contrastRatio: Math.max(...contrastRatios),
  };
}

function analyzeRingBoundary(data, size, radius, sampleOffset) {
  const pixelAt = (x, y) => {
    const offset = (y * size + x) * 3;
    return luminance(data.subarray(offset, offset + 3));
  };
  const center = (size - 1) / 2;
  const differences = [];

  for (let degrees = 0; degrees < 360; degrees += 5) {
    const angle = (degrees * Math.PI) / 180;
    const luminanceAtOffset = (radialOffset) => {
      const scaledRadius = ((radius + radialOffset) * size) / 1024;
      const x = Math.round(center + Math.cos(angle) * scaledRadius);
      const y = Math.round(center + Math.sin(angle) * scaledRadius);
      return pixelAt(x, y);
    };

    differences.push(
      Math.abs(
        luminanceAtOffset(-sampleOffset) - luminanceAtOffset(sampleOffset),
      ),
    );
  }

  return differences.reduce((sum, value) => sum + value, 0) / differences.length;
}

export async function checkIconArtwork(sourcePath = defaultSourcePath) {
  const failures = [];
  const metrics = [];
  const sourceSvg = await readFile(sourcePath, "utf8");
  const arrowPath = sourceSvg.match(/<path\b(?=[^>]*\bd="M243 738)[^>]*\/>/)?.[0];
  if (!arrowPath) {
    throw new Error(`Could not find the rotating disc arrow in ${sourcePath}`);
  }
  const withoutArrow = Buffer.from(sourceSvg.replace(arrowPath, ""));

  for (const size of iconSizes) {
    const [raster, arrowlessRaster] = await Promise.all([
      sharp(sourcePath)
        .resize(size, size, { fit: "contain" })
        .removeAlpha()
        .raw()
        .toBuffer({ resolveWithObject: true }),
      sharp(withoutArrow)
        .resize(size, size, { fit: "contain" })
        .removeAlpha()
        .raw()
        .toBuffer({ resolveWithObject: true }),
    ]);
    const { data, info } = raster;

    if (info.width !== size || info.height !== size || info.channels !== 3) {
      throw new Error(`Could not rasterize ${sourcePath} as an RGB ${size}px icon`);
    }

    const play = analyzePlaySymbol(data, size);
    metrics.push(
      `${size}px play symbol: ${play.visiblePixels} high-contrast pixels, ` +
        `${play.contrastRatio.toFixed(1)}:1 peak contrast`,
    );
    if (play.visiblePixels < minimumPlayPixels[size]) {
      failures.push(
        `${size}px play triangle has only ${play.visiblePixels} clearly separated ` +
          `pixel(s); expected at least ${minimumPlayPixels[size]}`,
      );
    }

    const arrowPixels = data.reduce((count, channel, index) => {
      if (index % 3 !== 0) return count;
      const difference =
        Math.abs(data[index] - arrowlessRaster.data[index]) +
        Math.abs(data[index + 1] - arrowlessRaster.data[index + 1]) +
        Math.abs(data[index + 2] - arrowlessRaster.data[index + 2]);
      return count + (difference >= 20 ? 1 : 0);
    }, 0);
    const minimumArrowPixelCount = minimumArrowPixels[size];
    metrics.push(
      `${size}px disc arrow: ${arrowPixels} pixels differ from the disc beneath it ` +
        `(minimum ${minimumArrowPixelCount})`,
    );
    if (arrowPixels < minimumArrowPixelCount) {
      failures.push(
        `${size}px disc arrow has only ${arrowPixels} pixels with clear visual ` +
          `separation; expected at least ${minimumArrowPixelCount}`,
      );
    }

    for (const { radius, minimum, label } of ringThresholds[size]) {
      const contrast = analyzeRingBoundary(
        data,
        size,
        radius,
        ringSampleOffsets[size],
      );
      metrics.push(
        `${size}px ${label}: ${contrast.toFixed(3)} mean edge contrast ` +
          `(minimum ${minimum.toFixed(3)})`,
      );
      if (contrast < minimum) {
        failures.push(
          `${size}px ${label} contrast is ${contrast.toFixed(3)}; ` +
            `expected at least ${minimum.toFixed(3)}`,
        );
      }
    }
  }

  return { failures, metrics };
}

export async function assertIconArtwork(sourcePath = defaultSourcePath) {
  const { failures, metrics } = await checkIconArtwork(sourcePath);
  if (failures.length > 0) {
    throw new Error(
      `Icon artwork check failed for ${sourcePath}:\n` +
        [...metrics, ...failures.map((failure) => `FAIL: ${failure}`)]
          .map((line) => `  ${line}`)
          .join("\n"),
    );
  }
  return metrics;
}

const isDirectExecution =
  process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isDirectExecution) {
  const sourcePath = process.argv[2]
    ? resolve(process.cwd(), process.argv[2])
    : defaultSourcePath;
  try {
    const metrics = await assertIconArtwork(sourcePath);
    console.log(`Icon artwork check passed for ${sourcePath}:\n${metrics.join("\n")}`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
