import type { DeliveryStage } from "../types";

interface StatusPillProps {
  stage: DeliveryStage | string;
}

const stageLabel: Record<string, string> = {
  planned: "Planned",
  scaffolded: "Scaffolded",
  partial: "Partial",
  not_implemented: "Not Implemented",
  success: "Success",
  empty: "Empty",
  error: "Error"
};

export function StatusPill({ stage }: StatusPillProps) {
  return (
    <span className={`status-pill status-pill--${stage}`}>
      {stageLabel[stage] ?? stage}
    </span>
  );
}

