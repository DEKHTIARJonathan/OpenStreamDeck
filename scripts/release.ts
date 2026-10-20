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
  linux: [".deb", ".deb.sig", ".AppImage", ".AppImage.sig"],
  macos: [".dmg", ".app.tar.gz", ".app.tar.gz.sig"],
  windows: [".msi", ".exe", ".msi.sig", ".exe.sig"],
} as const;
type Platform = keyof typeof extensions;

function updaterPublicKey() {
  const key = process.env.TAURI_SIGNING_PUBLIC_KEY?.trim() || "";
  const decoded = Buffer.from(key, "base64");
  const lines = decoded.toString("utf8").trimEnd().split(/\r?\n/);
  const packet = Buffer.from(lines[1] || "", "base64");
  if (
    !key ||
    decoded.toString("base64") !== key ||
    lines.length !== 2 ||
    !lines[0].startsWith("untrusted comment: ") ||
    packet.length !== 42 ||
    packet.subarray(0, 2).toString() !== "Ed"
  )
    throw new Error(
      "Missing or invalid TAURI_SIGNING_PUBLIC_KEY. Supply the Tauri .pub file content.",
    );
  return { key, packet };
}

async function configureUpdater() {
  const { key } = updaterPublicKey();
  for (const name of [
    "TAURI_SIGNING_PRIVATE_KEY",
    "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
  ]) {
    const value = process.env[name];
    if (
      value === undefined ||
      (name === "TAURI_SIGNING_PRIVATE_KEY" && !value.trim()) ||
      value.startsWith("REPLACE_WITH_") ||
      ["<key>", "<password>"].includes(value)
    )
      throw new Error(`Missing ${name}. Configure the release signing secret.`);
  }
  const directory = process.env.RUNNER_TEMP;
  if (!directory)
    throw new Error("RUNNER_TEMP is required for release configuration.");
  // Only public configuration is written. The CLI receives signing secrets from CI.
  await writeFile(
    join(directory, "updater-config.json"),
    `${JSON.stringify({ bundle: { createUpdaterArtifacts: true }, plugins: { updater: { pubkey: key } } }, null, 2)}\n`,
  );
}

async function readSignature(artifact: string) {
  const signature = (await readFile(`${artifact}.sig`, "utf8")).trim();
  const decoded = Buffer.from(signature, "base64");
  const lines = decoded.toString("utf8").trimEnd().split(/\r?\n/);
  const packet = Buffer.from(lines[1] || "", "base64");
  if (
    !signature ||
    signature.length > 16_384 ||
    decoded.toString("base64") !== signature ||
    lines.length !== 4 ||
    !lines[0].startsWith("untrusted comment: ") ||
    packet.length !== 74 ||
    !["Ed", "ED"].includes(packet.subarray(0, 2).toString()) ||
    !lines[2].startsWith("trusted comment: ") ||
    Buffer.from(lines[3], "base64").length !== 64
  )
    throw new Error(
      `Invalid updater signature file: ${basename(artifact)}.sig`,
    );
  const { packet: publicPacket } = updaterPublicKey();
  if (!packet.subarray(2, 10).equals(publicPacket.subarray(2, 10)))
    throw new Error(
      `Updater signing key does not match TAURI_SIGNING_PUBLIC_KEY: ${basename(artifact)}`,
    );
  return signature;
}

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
    if (names.filter((name) => name.endsWith(extension)).length !== 1)
      throw new Error(`Expected exactly one release asset: ${extension}`);
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
      "Unexpected release file; only flat installer, updater and signature assets are allowed.",
    );
  }
  if (new Set(names).size !== names.length)
    throw new Error("Duplicate asset filename.");
  for (const name of names.filter((name) => name.endsWith(".sig"))) {
    if (!names.includes(name.slice(0, -4)))
      throw new Error(`Signature has no matching artifact: ${name}`);
  }
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
  for (const file of files) {
    if (!(await readFile(file)).length)
      throw new Error(`Empty release asset: ${basename(file)}`);
    if (file.endsWith(".sig")) await readSignature(file.slice(0, -4));
  }
  await mkdir(output, { recursive: true });
  for (const file of files) await copyFile(file, join(output, basename(file)));
}

async function updaterManifest(names: string[], output: string) {
  await checkVersion();
  const repository = process.env.GITHUB_REPOSITORY || "";
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository))
    throw new Error("GITHUB_REPOSITORY must identify the release repository.");
  const tag = process.env.RELEASE_TAG!;
  const platforms: Record<string, { url: string; signature: string }> = {};
  const targets = {
    ".AppImage": ["linux-x86_64", "linux-x86_64-appimage"],
    ".deb": ["linux-x86_64-deb"],
    ".app.tar.gz": ["darwin-x86_64", "darwin-aarch64"],
    ".exe": ["windows-x86_64", "windows-x86_64-nsis"],
    ".msi": ["windows-x86_64-msi"],
  };
  for (const [extension, keys] of Object.entries(targets)) {
    const name = names.find((name) => name.endsWith(extension))!;
    const signature = await readSignature(join(output, name));
    for (const key of keys)
      platforms[key] = {
        url: `https://github.com/${repository}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`,
        signature,
      };
  }
  return {
    version: tag.slice(1),
    notes: `OpenStreamDeck ${tag}. See the GitHub release notes for details.`,
    pub_date: new Date().toISOString(),
    platforms,
  };
}

export async function verify(output = "release-assets") {
  const names = (await readdir(output))
    .filter((name) => name !== "SHA256SUMS" && name !== "latest.json")
    .sort();
  expectedAssets("all", names);
  const manifest = await updaterManifest(names, output);
  await writeFile(
    join(output, "latest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  names.push("latest.json");
  names.sort();
  const hashes: string[] = [];
  for (const name of names) {
    const bytes = await readFile(join(output, name));
    if (!bytes.length) throw new Error(`Empty release asset: ${name}`);
    hashes.push(`${createHash("sha256").update(bytes).digest("hex")}  ${name}`);
  }
  await writeFile(join(output, "SHA256SUMS"), `${hashes.join("\n")}\n`);
  return names;
}

async function publish() {
  await checkVersion();
  const names = await verify();
  const tag = process.env.RELEASE_TAG!;
  const notes = `OpenStreamDeck ${tag}\n\nInstallers: Linux DEB/AppImage, macOS universal DMG (Apple Silicon and Intel), Windows MSI/NSIS.\n\nSigned updater artifacts, .sig files and latest.json accompany the installers. The macOS .app.tar.gz is the universal updater payload; Linux updater payloads cover DEB and AppImage. In-app update controls are planned; this release prepares the signed feed.\n\nmacOS is ad-hoc signed, not notarized. Windows installers have no Authenticode signature. Updater signatures are separate from OS distribution signing. These builds do not include vendor firmware or proprietary drivers. Linux USB access requires the bundled udev rule (automatically installed by the DEB; see docs/CI.md for AppImage setup).\n\nSHA256SUMS covers installers, updater artifacts, signatures and latest.json. Automated tests use simulated device I/O and do not certify physical hardware behavior.\n`;
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
  gh([
    "release",
    "edit",
    tag,
    "--draft=false",
    ...(tag.includes("-") ? ["--latest=false"] : []),
  ]);
}

if (import.meta.main) {
  try {
    switch (process.argv[2]) {
      case "configure":
        await configureUpdater();
        break;
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
        throw new Error(
          "Expected configure, version, stage, verify or publish.",
        );
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}
