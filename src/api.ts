import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  defaultConfig,
  validateConfig,
  type Snapshot,
  type Config,
} from "./model";
export const native = isTauri();
export async function call<T = void>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (native) return invoke<T>(command, args);
  if (command === "snapshot") {
    const stored = localStorage.getItem("openstreamdeck-preview");
    const config = stored
      ? validateConfig(JSON.parse(stored))
      : defaultConfig();
    return {
      navigation: { view: "Deck", revision: 0 },
      page_context: {
        revision: 0,
        desired_page: config.active_page,
        applied_page: null,
        reason: "manual",
        application: null,
        error:
          "Browser preview · Application detection is available in the desktop app.",
        automatic: false,
        override_active: false,
        overlay: false,
      },
      config,
      device: {
        connected: false,
        serial: "",
        product: "N3",
        message: "Browser preview · USB requires the desktop app",
        packets_in: 0,
        packets_out: 0,
        sleeping: false,
      },
      agents: [],
      adapter_errors: [],
      data_dir: "Browser preview storage",
    } as Snapshot as T;
  }
  if (command === "save_config") {
    localStorage.setItem(
      "openstreamdeck-preview",
      JSON.stringify(validateConfig(args?.config as Config)),
    );
    return undefined as T;
  }
  throw new Error(
    "This action requires the desktop app. Launch it with cargo tauri dev.",
  );
}
export async function on<T>(event: string, fn: (payload: T) => void) {
  if (!native) return () => {};
  return listen<T>(event, (e) => fn(e.payload));
}
