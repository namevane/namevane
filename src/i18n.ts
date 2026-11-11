import bg from "./locales/bg.json";
import en from "./locales/en.json";

export type Language = "en" | "bg";

type Catalog = typeof en;

export const languages: { code: Language; name: string }[] = [
  { code: "en", name: "English" },
  { code: "bg", name: "Български" },
];

const catalogs: Record<Language, Catalog> = { en, bg };

let current: Language = "en";

export function language(): Language {
  return current;
}

export function setLanguage(code: string): Language {
  current = code === "bg" ? "bg" : "en";
  return current;
}

export function t(key: string, vars?: Record<string, string | number>): string {
  const text = read(catalogs[current], key) ?? read(catalogs.en, key) ?? key;
  if (!vars) {
    return text;
  }
  return Object.entries(vars).reduce(
    (result, [name, value]) => result.split(`{${name}}`).join(String(value)),
    text,
  );
}

function read(catalog: Catalog, key: string): string | null {
  let value: unknown = catalog;
  for (const part of key.split(".")) {
    if (!value || typeof value !== "object" || !(part in value)) {
      return null;
    }
    value = (value as Record<string, unknown>)[part];
  }
  return typeof value === "string" ? value : null;
}
