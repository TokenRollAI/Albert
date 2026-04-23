import { Icon } from "./Icon";
import type { ThemeMode } from "../types";

interface TopBarProps {
  workspace: string;
  theme: ThemeMode;
  onToggleTheme: () => void;
  onImportClick: () => void;
  onMockServerClick: () => void;
  onProvidersClick: () => void;
  gatewayRunning: boolean;
  gatewayBind: string | null;
}

export function TopBar({
  workspace,
  theme,
  onToggleTheme,
  onImportClick,
  onMockServerClick,
  onProvidersClick,
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
