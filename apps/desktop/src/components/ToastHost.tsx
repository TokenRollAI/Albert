import { Icon } from "./Icon";
import type { Toast } from "../hooks/useToasts";

interface ToastHostProps {
  toasts: Toast[];
  onDismiss: (id: string) => void;
}

export function ToastHost({ toasts, onDismiss }: ToastHostProps) {
  if (toasts.length === 0) return null;
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast toast--${toast.level}`}>
          <Icon name={iconFor(toast.level)} size={14} />
          <span className="toast__message">{toast.message}</span>
          <button
            type="button"
            className="btn btn--icon btn--icon-sm"
            onClick={() => onDismiss(toast.id)}
            aria-label="Dismiss"
          >
            <Icon name="close" size={12} />
          </button>
        </div>
      ))}
    </div>
  );
}

function iconFor(level: Toast["level"]) {
  switch (level) {
    case "success":
      return "zap" as const;
    case "warn":
      return "info" as const;
    case "error":
      return "info" as const;
    default:
      return "info" as const;
  }
}
