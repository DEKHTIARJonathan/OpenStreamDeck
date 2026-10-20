import { test, expect } from "@playwright/test";

test("sub-agent focus keeps child identity and displays adapter failures", async ({
  page,
}) => {
  await page.route(/\/src\/api\.ts(?:\?.*)?$/, (route) =>
    route.fulfill({
      contentType: "application/javascript",
      body: `
    import { defaultConfig } from '/src/model.ts';
    export const native = false;
    const agent = (id, extra={}) => ({id:'codex:'+id,source:'codex',agent:'codex',title:id,
      status:'ready',raw_status:'idle',session_id:id,target:id,updated_at:1,
      parent_session_id:null,is_subagent:false,can_focus:true,can_prompt:false,...extra});
    export async function call(command,args) {
      if(command==='snapshot') return {config:defaultConfig(),device:{connected:false,serial:'',product:'N3',message:'Fixture',packets_in:0,packets_out:0,sleeping:false},adapter_errors:[],data_dir:'fixture',agents:[
        agent('parent'),agent('child',{title:'Reviewer',parent_session_id:'parent',is_subagent:true,status:'working',raw_status:'active'}),
        agent('unavailable',{can_focus:false})]};
      if(command==='agent_focus') {window.__focus_calls=[...(window.__focus_calls||[]),args.id]; throw new Error('Fixture: originating window unavailable');}
      throw new Error('Unexpected fixture call: '+command);
    }
    export async function on() {return ()=>{};}
  `,
    }),
  );
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: /^Agents/ })
    .click();
  await expect(
    page.getByText("Ready means between turns", { exact: false }),
  ).toBeVisible();
  await expect(
    page.locator(".status").filter({ hasText: "completed" }),
  ).toHaveCount(0);
  const child = page.locator(".agent-card").filter({
    has: page.getByRole("heading", { name: "Reviewer", exact: true }),
  });
  await expect(child).toContainText("Sub-agent");
  await expect(child).toContainText("Parent: parent");
  await child.getByRole("button", { name: "Focus", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "originating window unavailable",
  );
  expect(
    await page.evaluate(
      () => (window as unknown as { __focus_calls: string[] }).__focus_calls,
    ),
  ).toEqual(["codex:child"]);
  const unavailable = page.locator(".agent-card").filter({
    has: page.getByRole("heading", { name: "unavailable", exact: true }),
  });
  await expect(
    unavailable.getByRole("button", { name: "Focus", exact: true }),
  ).toBeDisabled();
});
