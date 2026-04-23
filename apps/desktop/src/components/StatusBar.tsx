interface StatusBarProps {
  runtime: string;
  collectionsCount: number;
  message: string;
  phase: string;
  mockRunning?: boolean;
  mockBind?: string | null;
}

export function StatusBar({
  runtime,
  collectionsCount,
  message,
  phase,
  mockRunning = false,
  mockBind = null
}: StatusBarProps) {
  const connected = runtime === "Tauri Runtime";
  return (
    <footer className="statusbar">
      <div className="statusbar__left">
        <span
          className={
            connected
              ? "statusbar__dot statusbar__dot--ok"
              : "statusbar__dot statusbar__dot--warn"
          }
          aria-hidden="true"
        />
        <span className="statusbar__runtime">
          {connected ? "connected" : "fallback"}
        </span>
        <span className="statusbar__sep">·</span>
        <span>
          {collectionsCount} collection{collectionsCount === 1 ? "" : "s"}
        </span>
        <span className="statusbar__sep">·</span>
        {mockRunning ? (
          <span className="statusbar__mock">
            <span className="statusbar__dot statusbar__dot--ok" aria-hidden="true" />
            mock http://{mockBind}
          </span>
        ) : (
          <span className="statusbar__mock statusbar__mock--idle">
            <span
              className="statusbar__dot statusbar__dot--idle"
              aria-hidden="true"
            />
            mock idle
          </span>
        )}
        <span className="statusbar__sep">·</span>
        <span className="statusbar__message" title={message}>
          {message}
        </span>
      </div>
      <div className="statusbar__right">
        <span className="statusbar__phase">{phase}</span>
      </div>
    </footer>
  );
}
