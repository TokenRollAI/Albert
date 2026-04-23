import type { PropsWithChildren, ReactNode } from "react";

interface PanelCardProps extends PropsWithChildren {
  eyebrow?: string;
  title: string;
  aside?: ReactNode;
  className?: string;
}

export function PanelCard({
  eyebrow,
  title,
  aside,
  className,
  children
}: PanelCardProps) {
  return (
    <section className={className ? `panel-card ${className}` : "panel-card"}>
      <header className="panel-card__header">
        <div>
          {eyebrow ? <p className="panel-card__eyebrow">{eyebrow}</p> : null}
          <h2>{title}</h2>
        </div>
        {aside ? <div className="panel-card__aside">{aside}</div> : null}
      </header>
      <div className="panel-card__body">{children}</div>
    </section>
  );
}
