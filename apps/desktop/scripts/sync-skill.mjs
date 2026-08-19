import { cp, mkdir, rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const desktopDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoDir = path.resolve(desktopDir, "..", "..");
const source = path.join(repoDir, "skills", "genshin-build-wiki");
const target = path.join(desktopDir, "resources", "rules", "genshin-build-wiki");

await rm(target, { recursive: true, force: true });
await mkdir(path.dirname(target), { recursive: true });
await cp(source, target, { recursive: true });
