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
  const draft = Object.fromEntries(
    Object.keys(incoming).map((key) => {
      const k = key as keyof Config;
      return [
        key,
        equal(current.base[k], current.draft[k])
          ? incoming[k]
          : current.draft[k],
      ];
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
