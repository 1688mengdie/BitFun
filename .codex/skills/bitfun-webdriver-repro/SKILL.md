---
name: bitfun-webdriver-repro
description: Use this skill when writing or updating BitFun desktop WebDriver/WDIO repro scripts, especially for flows like create project, open workspace, create session, and take screenshots in dev mode.
---

# BitFun WebDriver Repro

Write repro scripts against the real desktop app in `debug/dev` mode.

## Rules

- Do not use `release` unless the user explicitly asks for it.
- Prefer existing WDIO helpers and screenshot helpers under `tests/e2e/`.
- Treat screenshots as validation artifacts, not proof by themselves. Always assert the UI state that the screenshot is supposed to show.
- After taking screenshots, inspect them. If left nav still shows `暂无普通工作区`, the repro is wrong even if backend assertions passed.

## Critical pitfall

Opening a workspace by calling backend `open_workspace` directly from the test is usually wrong for frontend repro scripts.

- `window.__TAURI__.core.invoke('open_workspace', ...)` updates backend state.
- It does **not** guarantee frontend `workspaceManager` state is refreshed.
- If frontend state is stale, left nav may still show `暂无普通工作区`, and `FlowChatManager.currentWorkspacePath` may still point to the assistant workspace.
- Then clicking `+ Code` can create a session against the wrong workspace context.

For frontend-correct repros, open the workspace through the frontend state layer:

```ts
await browser.execute(async (workspacePath: string) => {
  const module = await import('/src/infrastructure/services/business/workspaceManager.ts');
  return module.workspaceManager.openWorkspace(workspacePath);
}, projectPath);
```

## Recommended flow

1. Create the test directory with Tauri/backend commands if needed.
2. Open the workspace through `workspaceManager.openWorkspace(...)`.
3. Wait until all three conditions are true:
   - `get_current_workspace` returns the target path
   - `get_opened_workspaces` contains the target path
   - left nav DOM shows the project label
4. Only then click the session creation button.
5. Wait for `.bitfun-session-scene` and `[data-testid="chat-input-container"]`.
6. Save screenshots for the key steps.

## Minimal assertions

After opening workspace, verify both backend state and DOM:

```ts
await browser.waitUntil(async () => {
  const currentWorkspace = await tauriInvoke<WorkspaceInfo | null>('get_current_workspace', {});
  const openedWorkspaces = await tauriInvoke<WorkspaceInfo[]>('get_opened_workspaces', {});
  const labelTexts = await browser.execute(() =>
    Array.from(document.querySelectorAll('.bitfun-nav-panel__workspace-item-label'))
      .map(element => element.textContent ?? '')
  );

  return currentWorkspace?.rootPath === projectPath
    && openedWorkspaces.some(workspace => workspace.rootPath === projectPath)
    && labelTexts.some(text => text.includes(projectName));
});
```

## Session creation note

Top-left `+ Code` / `+ Cowork` buttons depend on frontend workspace state. Do not click them before the workspace assertions above pass.

## Dev-mode prerequisites

- Ensure the debug app binary exists.
- Ensure Vite dev server is running.
- If the frontend shows stale dependency resolution errors, restart Vite with `--force`.

Typical dev server command:

```bash
TAURI_DEV_HOST=127.0.0.1 pnpm --dir src/web-ui exec vite --force --host 127.0.0.1 --port 1422
```

## Good outputs

- One startup screenshot
- One workspace-opened screenshot where the left nav shows the project
- One final session screenshot where the header and chat input match the target workspace
