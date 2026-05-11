import type { GenerationContext, ImportEndpointChange } from "../types";

export function importChangeGenerationContext(
  change: ImportEndpointChange
): GenerationContext | null {
  if (!change.reasons || change.reasons.length === 0) return null;
  const detailNote =
    change.details && change.details.length > 0
      ? ` Details: ${change.details.join("; ")}.`
      : "";
  return {
    note: `Re-import detected endpoint contract drift for ${change.method.toUpperCase()} ${change.path}: ${change.reasons.join(", ")}.${detailNote} Refresh the success mock so it stays aligned with the changed API contract.`
  };
}
