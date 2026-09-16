import { readFileSync } from "node:fs";

const html = readFileSync("website/index.html", "utf8");
const mode = process.argv[2] ?? "structure";
let observations;
if (mode === "structure") {
  observations = {
    exactlyOnePrimaryHeading: (html.match(/<h1(?:\s|>)/gi) ?? []).length === 1,
    languageDeclared: /<html\s[^>]*lang="en"/i.test(html),
    charsetDeclared: /<meta\s[^>]*charset="utf-8"/i.test(html),
    pageTitlePresent: /<title>[^<]+<\/title>/i.test(html),
    mainLandmarkPresent: /<main(?:\s|>)/i.test(html),
  };
} else if (mode === "links") {
  const ids = new Set([...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]));
  const targets = [...html.matchAll(/<a\s[^>]*href="([^"]+)"/g)].map((match) => match[1]);
  observations = {
    actionPresent: targets.length > 0,
    localTargetsResolve: targets.every((target) => target.startsWith("#") && ids.has(target.slice(1))),
    localStylesheetPresent: html.includes('href="style.css"') && readFileSync("website/style.css").length > 0,
  };
} else if (mode === "claims") {
  observations = {
    fictionalContextDeclared: /fictional/i.test(html),
    noScriptExecution: !/<script(?:\s|>)/i.test(html),
    noGuaranteedOutcomeClaim: !/guaranteed\s+(growth|conversion|revenue)|proven\s+winner/i.test(html),
  };
} else {
  console.error("Unknown fixture check; choose structure, links or claims.");
  process.exit(2);
}
const passed = Object.values(observations).every(Boolean);
console.log(JSON.stringify({
  check: mode, observations, passed, provenance: "OBSERVED",
  limitation: "Explicit fixture contract only; not comprehensive HTML/accessibility/performance validation or measured growth.",
}));
process.exit(passed ? 0 : 2);
