import type { PropsWithChildren, ReactNode } from "react";

interface PanelCardProps extends PropsWithChildren {
  eyebrow?: string;
  title: string;
  aside?: ReactNode;
}

export function PanelCard({
  eyebrow,
  title,
  aside,
  children
}: PanelCardProps) {
  return (
    <section className="panel-card">
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

