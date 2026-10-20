import type { Config } from "./model";

export interface ConfigDraft {
  base: Config;
  draft: Config;
}
const equal = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);
export const isDirty = ({ base, draft }: ConfigDraft) => !equal(base, draft);

/** Native brightness/page changes and save acknowledgements must not erase edits.
 * Keep locally edited fields; adopt remote values only for untouched fields.
 * A save acknowledgement may arrive after the user has made additional edits.
 */
export function receiveConfig(
  current: ConfigDraft | null,
  incoming: Config,
): ConfigDraft {
  if (!current) return { base: incoming, draft: incoming };
  const keys = new Set([
    ...Object.keys(incoming),
    ...Object.keys(current.base),
    ...Object.keys(current.draft),
  ]);
  const draft = Object.fromEntries(
    [...keys].flatMap((key) => {
      const k = key as keyof Config;
      const edited =
        Object.hasOwn(current.base, key) !==
          Object.hasOwn(current.draft, key) ||
        !equal(current.base[k], current.draft[k]);
      const source = edited ? current.draft : incoming;
      return Object.hasOwn(source, key) ? [[key, source[k]]] : [];
    }),
  ) as unknown as Config;
  // A local page edit may remove the page chosen by a concurrent device event.
  if (!draft.pages.some((page) => page.id === draft.active_page)) {
    draft.active_page = current.draft.active_page;
  }
  return { base: incoming, draft };
}

export function editConfig(
  current: ConfigDraft,
  edit: (config: Config) => void,
): ConfigDraft {
  const draft = structuredClone(current.draft);
  edit(draft);
  return { ...current, draft };
}
