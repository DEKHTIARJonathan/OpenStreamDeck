import { createHash } from "node:crypto";
import {
  copyFile,
  mkdir,
  readdir,
  readFile,
  writeFile,
} from "node:fs/promises";
import { basename, join } from "node:path";

export const extensions = {
  linux: [".deb", ".AppImage"],
  macos: [".dmg"],
  windows: [".msi", ".exe"],
} as const;
type Platform = keyof typeof extensions;

export function validateVersion(
  tag: string,
  refType: string,
  versions: string[],
) {
  if (
    refType !== "tag" ||
    !/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?$/.test(tag)
  ) {
    throw new Error(
      "Release requires a vMAJOR.MINOR.PATCH tag (optionally a prerelease), not a branch.",
    );
  }
  if (
    versions.length !== 3 ||
    versions.some((version) => `v${version}` !== tag)
  ) {
    throw new Error(
      `Tag ${tag} must match package.json, src-tauri/Cargo.toml and tauri.conf.json: ${versions.join(", ")}`,
    );
  }
}

export async function checkVersion() {
  const pkg = await Bun.file("package.json").json();
  const config = await Bun.file("src-tauri/tauri.conf.json").json();
  const cargo = Bun.TOML.parse(
    await Bun.file("src-tauri/Cargo.toml").text(),
  ) as { package: { version: string } };
  validateVersion(
    process.env.RELEASE_TAG || "",
    process.env.RELEASE_REF_TYPE || "",
    [pkg.version, cargo.package.version, config.version],
  );
}

export function expectedAssets(platform: Platform | "all", names: string[]) {
  const required =
    platform === "all"
      ? Object.values(extensions).flat()
      : extensions[platform];
  for (const extension of required) {
    if (!names.some((name) => name.endsWith(extension)))
      throw new Error(`Missing release asset: ${extension}`);
  }
  const known = Object.values(extensions).flat();
  if (
    names.some(
      (name) =>
        !known.some((extension) => name.endsWith(extension)) ||
        basename(name) !== name,
    )
  ) {
    throw new Error(
      "Unexpected release file; only flat installer assets are allowed.",
    );
  }
  if (new Set(names).size !== names.length)
    throw new Error("Duplicate asset filename.");
}

export async function stage(
  platform: Platform,
  root: string,
  output = "release-assets",
) {
  if (!Object.hasOwn(extensions, platform) || !root)
    throw new Error("Unknown platform or missing bundle root.");
  const files: string[] = [];
  async function visit(path: string) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      if (entry.isDirectory()) await visit(join(path, entry.name));
      else if (
        entry.isFile() &&
        extensions[platform].some((extension) => entry.name.endsWith(extension))
      )
        files.push(join(path, entry.name));
    }
  }
  await visit(root);
  expectedAssets(
    platform,
    files.map((file) => basename(file)),
  );
  await mkdir(output, { recursive: true });
  for (const file of files) await copyFile(file, join(output, basename(file)));
}

export async function verify(output = "release-assets") {
  const names = (await readdir(output))
    .filter((name) => name !== "SHA256SUMS")
    .sort();
  expectedAssets("all", names);
  const hashes: string[] = [];
  for (const name of names) {
    const bytes = await readFile(join(output, name));
    if (!bytes.length) throw new Error(`Empty installer: ${name}`);
    hashes.push(`${createHash("sha256").update(bytes).digest("hex")}  ${name}`);
  }
  await writeFile(join(output, "SHA256SUMS"), `${hashes.join("\n")}\n`);
  return names;
}

async function publish() {
  await checkVersion();
  const names = await verify();
  const tag = process.env.RELEASE_TAG!;
  const notes = `OpenStreamDeck ${tag}\n\nInstallers: Linux DEB/AppImage, macOS universal DMG (Apple Silicon and Intel), Windows MSI/NSIS.\n\nmacOS is ad-hoc signed, not notarized. Windows installers are unsigned. These builds do not include vendor firmware or proprietary drivers. Linux USB access requires the bundled udev rule (automatically installed by the DEB; see docs/CI.md for AppImage setup).\n\nSHA256SUMS accompanies the installers. Automated tests use simulated device I/O and do not certify physical hardware behavior.\n`;
  await writeFile("release-notes.txt", notes);
  function gh(args: string[]) {
    const result = Bun.spawnSync(["gh", ...args], {
      stdout: "inherit",
      stderr: "inherit",
    });
    if (result.exitCode !== 0)
      throw new Error(
        `GitHub release command failed (${result.exitCode}); any draft is left unpublished for inspection.`,
      );
  }
  // All installers have already built and passed verification. A failed upload
  // leaves only a draft. Do not replace assets on an existing published release.
  gh([
    "release",
    "create",
    tag,
    "--verify-tag",
    "--draft",
    "--title",
    `OpenStreamDeck ${tag}`,
    "--notes-file",
    "release-notes.txt",
    ...(tag.includes("-") ? ["--prerelease"] : []),
    ...[...names, "SHA256SUMS"].map((name) => join("release-assets", name)),
  ]);
  gh(["release", "edit", tag, "--draft=false"]);
}

if (import.meta.main) {
  try {
    switch (process.argv[2]) {
      case "version":
        await checkVersion();
        break;
      case "stage":
        await stage(
          process.env.RELEASE_PLATFORM as Platform,
          process.env.RELEASE_BUNDLE_ROOT || "",
        );
        break;
      case "verify":
        await verify();
        break;
      case "publish":
        await publish();
        break;
      default:
        throw new Error("Expected version, stage, verify or publish.");
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}
