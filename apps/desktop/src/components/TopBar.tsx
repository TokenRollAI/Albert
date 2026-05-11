import { Icon } from "./Icon";
import type { ThemeMode } from "../types";

interface TopBarProps {
  workspace: string;
  theme: ThemeMode;
  onToggleTheme: () => void;
  onWorkspaceClick: () => void;
  onImportClick: () => void;
  onMockServerClick: () => void;
  onProvidersClick: () => void;
  onExportAll: () => void;
  canExportAll: boolean;
  gatewayRunning: boolean;
  gatewayBind: string | null;
}

export function TopBar({
  workspace,
  theme,
  onToggleTheme,
  onWorkspaceClick,
  onImportClick,
  onMockServerClick,
  onProvidersClick,
  onExportAll,
  canExportAll,
  gatewayRunning,
  gatewayBind
}: TopBarProps) {
  return (
    <header className="topbar">
      <div className="topbar__brand">
        <img
          src="/favicon-32x32.png"
          alt="Albert"
          className="topbar__logo"
        />
        <span className="topbar__product">Albert</span>
        <span className="topbar__sep">/</span>
        <span className="topbar__workspace" title={workspace}>
          {workspace}
        </span>
        <button
          type="button"
          className="topbar__workspace-btn"
          onClick={onWorkspaceClick}
          title="Open workspace collections"
          aria-label="Open workspace collections"
        >
          <Icon name="database" size={13} />
        </button>
      </div>

      <div className="topbar__actions">
        <button
          type="button"
          className="btn btn--primary"
          onClick={onImportClick}
        >
          <Icon name="import" size={14} />
          <span>Import</span>
        </button>

        <button
          type="button"
          className={
            gatewayRunning
              ? "btn btn--secondary btn--pulse"
              : "btn btn--secondary"
          }
          onClick={onMockServerClick}
          title={
            gatewayRunning
              ? `Mock server running at ${gatewayBind}`
              : "Open mock server panel"
          }
        >
          <Icon name="server" size={14} />
          <span>
            {gatewayRunning && gatewayBind ? gatewayBind : "Mock Server"}
          </span>
        </button>

        <button
          type="button"
          className="btn btn--secondary"
          onClick={onProvidersClick}
          title="Provider configuration"
        >
          <Icon name="sparkles" size={14} />
          <span>Providers</span>
        </button>

        <button
          type="button"
          className="btn btn--icon"
          onClick={onExportAll}
          disabled={!canExportAll}
          aria-label="Export all collections"
          title="Export all collections"
        >
          <Icon name="save" size={16} />
        </button>

        <button
          type="button"
          className="btn btn--icon"
          onClick={onToggleTheme}
          aria-label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
          title={theme === "dark" ? "Light theme" : "Dark theme"}
        >
          <Icon name={theme === "dark" ? "sun" : "moon"} size={16} />
        </button>
      </div>
    </header>
  );
}
