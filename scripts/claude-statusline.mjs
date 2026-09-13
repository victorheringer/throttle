import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";

const CACHE_DIR = path.join(process.env.APPDATA, "com.victorheringer.usagetracker");
const CACHE_FILE = path.join(CACHE_DIR, "claude-code-rate-limits.json");

function readStdin() {
  try {
    return readFileSync(0, "utf-8");
  } catch {
    return "";
  }
}

function main() {
  const raw = readStdin();
  let input;
  try {
    input = JSON.parse(raw);
  } catch {
    process.stdout.write("");
    return;
  }

  const rateLimits = input.rate_limits ?? {};
  const fiveHour = rateLimits.five_hour ?? null;
  const sevenDay = rateLimits.seven_day ?? null;

  try {
    mkdirSync(CACHE_DIR, { recursive: true });
    writeFileSync(
      CACHE_FILE,
      JSON.stringify({
        five_hour: fiveHour,
        seven_day: sevenDay,
        written_at: Date.now(),
      }),
    );
  } catch {
    // the cache file is best-effort; it should never break the statusline
  }

  const parts = [];
  if (fiveHour && typeof fiveHour.used_percentage === "number") {
    parts.push(`5h: ${Math.round(fiveHour.used_percentage)}%`);
  }
  if (sevenDay && typeof sevenDay.used_percentage === "number") {
    parts.push(`7d: ${Math.round(sevenDay.used_percentage)}%`);
  }

  process.stdout.write(parts.join(" · "));
}

main();
