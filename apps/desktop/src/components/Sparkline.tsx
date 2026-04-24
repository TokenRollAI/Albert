export interface SparkBucket {
  minuteEpochMs: number;
  count: number;
  status5xx: number;
}

/**
 * Tiny per-minute bar chart. Each bar's height is proportional to the
 * peak count in the visible window; a 5xx share is overlaid in an
 * error-tinted segment at the bar's base. No axes, no legend — this is
 * a glanceable "is traffic arriving" indicator, not a full dashboard.
 *
 * Lives in its own module so the Requests-tab file can focus on
 * filter / list rendering, and so other surfaces can drop in a spark
 * chart without pulling in the whole request-log tree.
 */
export function Sparkline({ buckets }: { buckets: SparkBucket[] }) {
  const peak = Math.max(1, ...buckets.map((b) => b.count));
  return (
    <div
      className="sparkline"
      role="img"
      aria-label={`Request rate over the last ${buckets.length} minutes; peak ${peak}/min`}
    >
      {buckets.map((bucket, idx) => {
        const height = Math.round((bucket.count / peak) * 100);
        const errHeight =
          bucket.count === 0
            ? 0
            : Math.round((bucket.status5xx / bucket.count) * height);
        const minute = new Date(bucket.minuteEpochMs).toLocaleTimeString(
          undefined,
          { hour12: false, minute: "2-digit", hour: "2-digit" }
        );
        const title = `${minute} — ${bucket.count} req${
          bucket.count === 1 ? "" : "s"
        }${bucket.status5xx ? ` (${bucket.status5xx} 5xx)` : ""}`;
        return (
          <span
            key={`${bucket.minuteEpochMs}-${idx}`}
            className="sparkline__bar"
            style={{ height: `${Math.max(height, 2)}%` }}
            title={title}
          >
            {errHeight > 0 ? (
              <span
                className="sparkline__bar-err"
                style={{ height: `${errHeight}%` }}
              />
            ) : null}
          </span>
        );
      })}
    </div>
  );
}
