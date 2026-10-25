import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  assertIconArtwork,
  checkIconArtwork,
} from "./check-icon-artwork.mjs";

const sourcePath = fileURLToPath(
  new URL("../src-tauri/icons/icon.svg", import.meta.url),
);
const temporaryDirectory = await mkdtemp(join(tmpdir(), "bdmv-icon-check-"));
const originalSvg = await readFile(sourcePath, "utf8");

after(async () => {
  await rm(temporaryDirectory, { recursive: true, force: true });
});

async function writeVariant(name, svg) {
  const path = join(temporaryDirectory, name);
  await writeFile(path, svg);
  return path;
}

test("the source icon keeps the play symbol and disc details at 16px, 32px, 48px, and 64px", async () => {
  const { failures, metrics } = await checkIconArtwork(sourcePath);
  assert.deepEqual(failures, []);
  assert.equal(metrics.length, 24);
});

test("the check rejects a missing play triangle at 48px and 64px", async () => {
  const withoutTriangle = originalSvg.replace(
    /<path d="M482 443[\s\S]*?fill="#ddfaff"\/>/,
    "",
  );
  assert.notEqual(withoutTriangle, originalSvg, "the play path should be found");
  const variantPath = await writeVariant("without-play.svg", withoutTriangle);

  await assert.rejects(assertIconArtwork(variantPath), (error) => {
    assert.match(error.message, /48px play triangle/);
    assert.match(error.message, /64px play triangle/);
    return true;
  });
});

test("the check rejects a missing disc arrow", async () => {
  const withoutArrow = originalSvg.replace(
    /<path\b(?=[^>]*\bd="M243 738)[^>]*\/>/,
    "",
  );
  assert.notEqual(withoutArrow, originalSvg, "the disc arrow should be found");
  const variantPath = await writeVariant("without-arrow.svg", withoutArrow);

  await assert.rejects(assertIconArtwork(variantPath), /rotating disc arrow/);
});

test("the check rejects a flattened disc arrow at 48px and 64px", async () => {
  const flattenedArrow = originalSvg.replace(
    /(<path\b(?=[^>]*\bd="M243 738)[^>]*?)fill="url\(#disc\)" stroke="#07213b" stroke-width="14" stroke-linejoin="round"\/>/,
    '$1fill="none" stroke="none"/>',
  );
  assert.notEqual(flattenedArrow, originalSvg, "the disc arrow should be found");
  const variantPath = await writeVariant("flattened-arrow.svg", flattenedArrow);

  await assert.rejects(assertIconArtwork(variantPath), (error) => {
    assert.match(error.message, /48px disc arrow/);
    assert.match(error.message, /64px disc arrow/);
    return true;
  });
});

test("the check rejects flattened disc rings at 48px and 64px", async () => {
  const withoutRings = originalSvg
    .replace(/<circle cx="512" cy="512" r="248"[^>]*\/>/, "")
    .replace(/<circle cx="512" cy="512" r="196"[^>]*\/>/, "");
  assert.notEqual(withoutRings, originalSvg, "disc rings should be found");
  const variantPath = await writeVariant("without-rings.svg", withoutRings);

  await assert.rejects(assertIconArtwork(variantPath), (error) => {
    assert.match(error.message, /48px (?:outer disc ring|bright disc ring|inner disc edge)/);
    assert.match(error.message, /64px (?:outer disc ring|bright disc ring|inner disc edge)/);
    return true;
  });
});
