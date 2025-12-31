#!/usr/bin/env node

import fs from "fs";
import path from "path";
import crypto from "crypto";
import readline from "readline";
import { XMLParser } from "fast-xml-parser";
import TurndownService from "turndown";
import mime from "mime-types";

/* ============================= Configuration ============================= */

const MAX_NOTES = 5000;
const FAILURE_RATE_THRESHOLD = 0.3;
const AI_TIMEOUT_MS = 60_000;

const USE_AI_TAGGING = process.env.USE_AI_TAGGING === "true";
const USE_AI_CLEANUP = process.env.USE_AI_CLEANUP === "true";

const OPENAI_API_BASE =
  process.env.OPENAI_API_BASE || "https://api.openai.com/v1";
const OPENAI_API_KEY = process.env.OPENAI_API_KEY;
const OPENAI_MODEL =
  process.env.OPENAI_MODEL || "gpt-4o-mini";

if ((USE_AI_TAGGING || USE_AI_CLEANUP) && !OPENAI_API_KEY) {
  throw new Error(
    "OPENAI_API_KEY must be set when AI features are enabled"
  );
}

/* ================================ Logging ================================ */

function ts() {
  return new Date().toISOString();
}

function log(msg) {
  console.log(`[${ts()}] [evernote2jottery] ${msg}`);
}

function warn(msg) {
  console.warn(`[${ts()}] [evernote2jottery] ⚠️  ${msg}`);
}

function bail(msg) {
  console.error(`[${ts()}] [evernote2jottery] ❌ ${msg}`);
  process.exit(1);
}

/* ================================ Helpers ================================ */

function sanitize(text) {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9-_]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^[-_]+|[-_]+$/g, "");
}

function shortId() {
  return crypto.randomBytes(3).toString("hex");
}

function evernoteDateToISO(dateStr) {
  return `${dateStr.slice(0, 4)}-${dateStr.slice(4, 6)}-${dateStr.slice(
    6,
    8
  )}T${dateStr.slice(9, 11)}:${dateStr.slice(
    11,
    13
  )}:${dateStr.slice(13, 15)}.000Z`;
}

function parseJsonFromModel(text) {
  const cleaned = text
    .trim()
    .replace(/^```(?:json)?/i, "")
    .replace(/```$/, "")
    .trim();
  return JSON.parse(cleaned);
}

async function fetchWithTimeout(url, options) {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), AI_TIMEOUT_MS);

  try {
    return await fetch(url, { ...options, signal: controller.signal });
  } finally {
    clearTimeout(id);
  }
}

async function promptContinue(message) {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  return new Promise((resolve) => {
    rl.question(message, (answer) => {
      rl.close();
      resolve(answer.trim().toLowerCase() === "y");
    });
  });
}

/* ============================ ENML Utilities ============================= */

function replaceEnMediaWithPlaceholders(enml) {
  return enml.replace(
    /<en-media\b([^>]*)\/?>/gi,
    (match, attrs) => {
      const typeMatch = attrs.match(/type="([^"]+)"/i);
      const hashMatch = attrs.match(/hash="([^"]+)"/i);

      const type = typeMatch
        ? typeMatch[1]
        : "application/octet-stream";
      const hash = hashMatch ? hashMatch[1] : "unknown";

      return `[[ATTACHMENT:${type}:${hash}]]`;
    }
  );
}

/* =============================== AI Prompts ============================== */

const TAGGING_SYSTEM_PROMPT = `
You are assigning tags to a note.

Rules:
- Read and understand the note content.
- Return 1–4 concise, semantic tags.
- Empty arrays are acceptable if no tags can be inferred.
- Tags must be lowercase.
- Tags must contain only letters, numbers, dashes, or underscores.
- Do NOT invent unrelated tags.
- Do NOT explain your choices.
- Return ONLY a raw JSON array of strings.

Goal:
Produce high-quality organizational tags that describe the note's topic.
`.trim();

const CLEANUP_SYSTEM_PROMPT = `
You are cleaning and normalizing note content that contains attachment placeholders.

Rules:
- Preserve ALL information and meaning.
- Do NOT summarize, rewrite, or remove content.
- Do NOT add new content.
- Only reformat into clean, consistent Markdown.

Markdown formatting rules:
- Use Markdown headings (#, ##, ###) where appropriate.
- Normalize lists to "-" bullets.
- Preserve code blocks using fenced triple backticks.
- Add the language to fenced code blocks if it can be contextually inferred
  so syntax highlighting can be applied.
- Remove excessive blank lines.
- Ensure the document is valid Markdown.

Attachment handling:
- You will see placeholders in this form:
  [[ATTACHMENT:MIME_TYPE:HASH]]
- Replace each placeholder IN PLACE.
- Use:
  ![attachment](attachment:FILENAME) if MIME_TYPE starts with "image/"
  [Attachment: FILENAME](attachment:FILENAME) otherwise.
- Use the provided attachment mapping to find FILENAME.
- Do NOT move attachments to the end unless no logical inline position exists.
- If needed, append them under:
  ## Attachments

Output:
- Return ONLY the cleaned Markdown.
- No explanations.
- No code fences around the output.
`.trim();

/* ============================ ENEX Parsing =============================== */

function parseEnex(filePath) {
  log("Reading ENEX file");
  const xml = fs.readFileSync(filePath, "utf8");

  const parser = new XMLParser({
    ignoreAttributes: false,
    attributeNamePrefix: "",
  });

  const parsed = parser.parse(xml);
  const root = parsed["en-export"];

  if (!root || !root.note) {
    bail("Invalid ENEX file");
  }

  const notes = Array.isArray(root.note)
    ? root.note
    : [root.note];

  if (notes.length > MAX_NOTES) {
    bail(`Too many notes (${notes.length})`);
  }

  log(`Found ${notes.length} notes`);
  return notes;
}

/* ================================ Main =================================== */

async function run() {
  const [, , inputPath] = process.argv;
  if (!inputPath) {
    bail("Usage: node evernote2jottery.js input.enex");
  }

  const importTag = sanitize(path.basename(inputPath, ".enex"));

  log(`Import tag: ${importTag}`);
  log(`AI tagging: ${USE_AI_TAGGING ? "ENABLED" : "DISABLED"}`);
  log(`AI cleanup: ${USE_AI_CLEANUP ? "ENABLED" : "DISABLED"}`);

  const enNotes = parseEnex(inputPath);
  const turndown = new TurndownService({
    codeBlockStyle: "fenced",
  });

  const notes = [];
  let aiAttempts = 0;
  let aiFailures = 0;

  for (let i = 0; i < enNotes.length; i++) {
    const en = enNotes[i];
    const title = en.title || "Untitled note";

    log(`Processing note ${i + 1}/${enNotes.length}: "${title}"`);

    const rawEnml = en.content
      .replace(/<!\[CDATA\[|\]\]>/g, "")
      .replace(/<\/?en-note[^>]*>/g, "");

    const html = replaceEnMediaWithPlaceholders(rawEnml);
    let content = turndown.turndown(html).trim();

    const resources = en.resource
      ? Array.isArray(en.resource)
        ? en.resource
        : [en.resource]
      : [];

    if (!content && resources.length === 0) {
      log(`Skipping empty note: "${title}"`);
      continue;
    }

    const attachmentMap = {};
    const attachments = resources.map((res) => {
      const ext = mime.extension(res.mime) || "bin";
      const filename = `attachment-${shortId()}.${ext}`;

      if (res.data?.hash) {
        attachmentMap[res.data.hash] = {
          filename,
          mimeType: res.mime,
        };
      }

      return {
        filename,
        mimeType: res.mime,
        data: res.data["#text"],
      };
    });

    let tags = [importTag];

    try {
      if (USE_AI_TAGGING) {
        aiAttempts++;
        log(`AI tagging → "${title}"`);

        const response = await fetchWithTimeout(
          `${OPENAI_API_BASE}/chat/completions`,
          {
            method: "POST",
            headers: {
              Authorization: `Bearer ${OPENAI_API_KEY}`,
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: OPENAI_MODEL,
              temperature: 0,
              messages: [
                { role: "system", content: TAGGING_SYSTEM_PROMPT },
                { role: "user", content: content.slice(0, 2000) },
              ],
            }),
          }
        );

        const data = await response.json();
        const aiTags = parseJsonFromModel(
          data.choices[0].message.content
        );

        tags = [
          importTag,
          ...aiTags.map(sanitize).filter(Boolean),
        ].slice(0, 5);
      }

      if (USE_AI_CLEANUP) {
        aiAttempts++;
        log(`AI cleanup → "${title}"`);

        const response = await fetchWithTimeout(
          `${OPENAI_API_BASE}/chat/completions`,
          {
            method: "POST",
            headers: {
              Authorization: `Bearer ${OPENAI_API_KEY}`,
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: OPENAI_MODEL,
              temperature: 0,
              messages: [
                { role: "system", content: CLEANUP_SYSTEM_PROMPT },
                {
                  role: "user",
                  content: `
Original Markdown:
${content}

Attachment mapping (JSON):
${JSON.stringify(attachmentMap, null, 2)}
`,
                },
              ],
            }),
          }
        );

        const data = await response.json();
        content = data.choices[0].message.content.trim();
      }
    } catch (err) {
      aiFailures++;
      warn(
        `AI failed for note ${i + 1} ("${title}")`
      );

      const rate = aiFailures / aiAttempts;
      if (rate > FAILURE_RATE_THRESHOLD) {
        const ok = await promptContinue(
          `AI failure rate ${(rate * 100).toFixed(
            1
          )}% (${aiFailures}/${aiAttempts}). Continue? (y/N): `
        );
        if (!ok) break;
      }
    }

    notes.push({
      id: crypto.randomUUID(),
      title,
      createdAt: evernoteDateToISO(en.created),
      modifiedAt: evernoteDateToISO(en.updated),
      content,
      tags,
      attachments,
      pinned: false,
      syntaxLanguage: "markdown",
    });
  }

  const outputPath = path.join(
    path.dirname(inputPath),
    `${path.basename(inputPath, ".enex")}.json`
  );

  fs.writeFileSync(
    outputPath,
    JSON.stringify(
      {
        version: "1.0",
        exportDate: new Date().toISOString(),
        notes,
      },
      null,
      2
    )
  );

  log(`✅ Wrote ${notes.length} notes`);
}

run().catch((err) => bail(err.stack || err.message));
