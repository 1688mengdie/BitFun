import { useEffect, useState } from 'react';
import { workspaceManager } from '../services/business/workspaceManager';

function readWorkspaceFields() {
  const ws = workspaceManager.getState().currentWorkspace;
  return {
    workspacePath: ws?.rootPath ?? '',
    hasWorkspace: !!ws,
  };
}

/**
 * Active workspace path and presence, synced from {@link workspaceManager}.
 * Use where React context may not match the app root provider (e.g. duplicate
 * context module in a lazy chunk); behavior aligns with WorkspaceProvider state.
 */
export function useWorkspaceManagerSync(): {
  workspacePath: string;
  hasWorkspace: boolean;
} {
  const [fields, setFields] = useState(readWorkspaceFields);

  useEffect(() => {
    return workspaceManager.addEventListener(() => {
      setFields(readWorkspaceFields());
    });
  }, []);

  return fields;
}
