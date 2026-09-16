import { useSyncExternalStore } from "react";
import {
  getLocale,
  setLocale as setParaglideLocale,
  type Locale,
} from "./paraglide/runtime.js";

const listeners = new Set<() => void>();

function applyDocumentLocale(locale: Locale): void {
  document.documentElement.lang = locale;
  document.documentElement.dir = locale === "ar" || locale === "fa" ? "rtl" : "ltr";
}

export async function setLocale(next: Locale): Promise<void> {
  if (next === getLocale()) return;
  await setParaglideLocale(next, { reload: false });
  applyDocumentLocale(next);
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useLocale(): Locale {
  return useSyncExternalStore(subscribe, getLocale, getLocale);
}
