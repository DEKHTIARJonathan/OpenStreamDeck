import { describe, it, expect } from "vitest";
import {
  configuredAction,
  changeActionType,
  assignDial,
  applicationAction,
} from "./action-defaults";
import { action, catalog, defaultConfig } from "./model";
describe("action assignment", () => {
  for (const [kind] of catalog)
    it(`initializes ${kind} without stale fields`, () => {
      const old = {
        ...action("agent_focus", "Focus", "selected"),
        options: { provider: "stale" },
        children: [action("shell", "Old command", "old")],
      };
      const next = changeActionType(old, kind);
      expect(next).toEqual({
        ...configuredAction(kind),
        color: old.color,
        image: old.image,
      });
      expect(next.children).toEqual([]);
    });
  for (const dial of ["knob1", "knob2", "knob3"])
    for (const preset of ["brightness", "volume", "agents"] as const)
      it(`${preset} maps all three triggers on ${dial}`, () => {
        const p = defaultConfig().pages[0];
        const before = structuredClone(p.bindings);
        assignDial(p, dial, preset);
        for (const trigger of [dial, `${dial}.ccw`, `${dial}.cw`])
          expect(p.bindings[trigger].kind).toBe(
            preset === "brightness"
              ? "brightness"
              : preset === "volume"
                ? "media"
                : trigger === dial
                  ? "agent_focus"
                  : "agent_select",
          );
        if (preset === "brightness") {
          expect(p.bindings[`${dial}.ccw`].value).toBe("-5");
          expect(p.bindings[`${dial}.cw`].value).toBe("+5");
          expect(p.bindings[dial].value).toBe("60");
        }
        for (const key of Object.keys(before).filter(
          (k) => !k.startsWith(dial),
        ))
          expect(p.bindings[key]).toEqual(before[key]);
      });
});

it("selecting an app replaces target, label and full-key artwork together", () => {
  const previous = {
    ...action("shell", "Old", "old"),
    options: { cwd: "/old" },
    children: [action()],
  };
  const chosen = applicationAction(previous, {
    name: "Calculator",
    path: "/System/Applications/Calculator.app",
    image: "data:image/png;base64,fixture",
  });
  expect(chosen).toMatchObject({
    kind: "open",
    title: "Calculator",
    value: "/System/Applications/Calculator.app",
    image: "data:image/png;base64,fixture",
    options: {},
    children: [],
  });
  expect(() =>
    applicationAction(previous, { name: "Missing", path: "", image: "" }),
  ).toThrow();
});
