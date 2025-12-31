#!/usr/bin/env node

import fs from "fs";
import path from "path";

/* ----------------------------- Helpers ----------------------------- */

function bail(msg) {
  console.error(`❌ ${msg}`);
  process.exit(1);
}

/* ------------------------------ Main ------------------------------- */

// argv looks like:
// [node, script, input1.json, input2.json, ..., output.json]
const args = process.argv.slice(2);

if (args.length < 2) {
  bail(
    "Usage: node merge-jottery-json.js *.json output.json"
  );
}

const outputFile = args[args.length - 1];
const inputFiles = args.slice(0, -1);

let allNotes = [];

for (const file of inputFiles) {
  if (!fs.existsSync(file)) {
    bail(`File not found: ${file}`);
  }

  const raw = fs.readFileSync(file, "utf8");
  let data;

  try {
    data = JSON.parse(raw);
  } catch {
    bail(`Invalid JSON: ${file}`);
  }

  if (data.version !== "1.0" || !Array.isArray(data.notes)) {
    bail(`Not a valid Jottery export: ${file}`);
  }

  console.log(
    `✔ Loaded ${data.notes.length} notes from ${path.basename(
      file
    )}`
  );

  allNotes.push(...data.notes);
}

const merged = {
  version: "1.0",
  exportDate: new Date().toISOString(),
  notes: allNotes,
};

fs.writeFileSync(outputFile, JSON.stringify(merged, null, 2));

console.log(
  `✅ Wrote ${allNotes.length} notes to ${outputFile}`
);
