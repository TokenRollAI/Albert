import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import { useDraftMap } from "../hooks/useDraftMap";
import type {
  ConditionalExampleRule,
  GatewayRouteSummary,
  MockExampleKind,
  RequestCondition
} from "../types";

interface ConditionalRulesEditorProps {
  running: boolean;
  routes: GatewayRouteSummary[];
  value: Record<string, ConditionalExampleRule[]>;
  onApply: (
    next: Record<string, ConditionalExampleRule[]>
  ) => Promise<void>;
}

type ConditionSource = RequestCondition["source"];

const EXAMPLE_KINDS: MockExampleKind[] = ["success", "empty", "error"];

export function ConditionalRulesEditor({
  running,
  routes,
  value,
  onApply
}: ConditionalRulesEditorProps) {
  const { draft, setDraft, dirty, busy, apply, reset } = useDraftMap(
    normalizeRules(value),
    async (next) => onApply(normalizeRules(next))
  );
  const [selectedRoute, setSelectedRoute] = useState<string>(() =>
    routes.length > 0 ? routeKeyOf(routes[0]) : ""
  );
  const [name, setName] = useState("Header branch");
  const [example, setExample] = useState<MockExampleKind>("success");
  const [source, setSource] = useState<ConditionSource>("query");
  const [field, setField] = useState("status");
  const [equals, setEquals] = useState("vip");

  const entries = useMemo(
    () =>
      Object.entries(draft)
        .map(([route, rules]) => ({ route, rules }))
        .filter(({ rules }) => rules.length > 0)
        .sort((a, b) => (a.route < b.route ? -1 : a.route > b.route ? 1 : 0)),
    [draft]
  );

  function addRule() {
    const route = selectedRoute.trim();
    const key = field.trim();
    if (!route || !key) return;
    const nextRule: ConditionalExampleRule = {
      name: name.trim() || `${source} match`,
      example,
      when: [makeCondition(source, key, equals)]
    };
    setDraft((prev) => ({
      ...prev,
      [route]: [...(prev[route] ?? []), nextRule]
    }));
  }

  function updateRule(
    route: string,
    index: number,
    patch: Partial<ConditionalExampleRule>
  ) {
    setDraft((prev) => {
      const next = { ...prev };
      const rules = [...(next[route] ?? [])];
      const current = rules[index];
      if (!current) return prev;
      rules[index] = { ...current, ...patch };
      next[route] = rules;
      return next;
    });
  }

  function removeRule(route: string, index: number) {
    setDraft((prev) => {
      const next = { ...prev };
      const rules = [...(next[route] ?? [])];
      rules.splice(index, 1);
      if (rules.length === 0) {
        delete next[route];
      } else {
        next[route] = rules;
      }
      return next;
    });
  }

  function moveRule(route: string, index: number, direction: -1 | 1) {
    setDraft((prev) => {
      const next = { ...prev };
      const rules = [...(next[route] ?? [])];
      const target = index + direction;
      if (target < 0 || target >= rules.length) return prev;
      const [rule] = rules.splice(index, 1);
      rules.splice(target, 0, rule);
      next[route] = rules;
      return next;
    });
  }

  function addCondition(route: string, index: number) {
    setDraft((prev) => {
      const next = { ...prev };
      const rules = [...(next[route] ?? [])];
      const current = rules[index];
      if (!current) return prev;
      rules[index] = {
        ...current,
        when: [...current.when, makeCondition("query", "status", "active")]
      };
      next[route] = rules;
      return next;
    });
  }

  function updateCondition(
    route: string,
    ruleIndex: number,
    conditionIndex: number,
    patch: Partial<EditableCondition>
  ) {
    setDraft((prev) => {
      const next = { ...prev };
      const rules = [...(next[route] ?? [])];
      const rule = rules[ruleIndex];
      if (!rule) return prev;
      const conditions = [...rule.when];
      const current = conditions[conditionIndex];
      if (!current) return prev;
      conditions[conditionIndex] = patchCondition(current, patch);
      rules[ruleIndex] = { ...rule, when: conditions };
      next[route] = rules;
      return next;
    });
  }

  function removeCondition(
    route: string,
    ruleIndex: number,
    conditionIndex: number
  ) {
    setDraft((prev) => {
      const next = { ...prev };
      const rules = [...(next[route] ?? [])];
      const rule = rules[ruleIndex];
      if (!rule) return prev;
      const conditions = [...rule.when];
      conditions.splice(conditionIndex, 1);
      if (conditions.length === 0) {
        rules.splice(ruleIndex, 1);
      } else {
        rules[ruleIndex] = { ...rule, when: conditions };
      }
      if (rules.length === 0) {
        delete next[route];
      } else {
        next[route] = rules;
      }
      return next;
    });
  }

  return (
    <section className="panel">
      <div className="panel__title panel__title--row">
        <h3>Conditional examples</h3>
        <span className="panel__meta">
          first match wins - query / header / body equality
        </span>
      </div>

      <div className="formgrid formgrid--conditional">
        <label className="field">
          <span className="field__label">Route</span>
          <select
            className="select"
            value={selectedRoute}
            onChange={(event) => setSelectedRoute(event.target.value)}
            disabled={!running || routes.length === 0}
          >
            {routes.length === 0 ? <option value="">No routes</option> : null}
            {routes.map((route) => {
              const key = routeKeyOf(route);
              return (
                <option key={key} value={key}>
                  {key}
                </option>
              );
            })}
          </select>
        </label>
        <label className="field">
          <span className="field__label">Rule name</span>
          <input
            type="text"
            value={name}
            onChange={(event) => setName(event.target.value)}
            disabled={!running}
            spellCheck={false}
          />
        </label>
        <label className="field">
          <span className="field__label">Example</span>
          <select
            className="select"
            value={example}
            onChange={(event) =>
              setExample(event.target.value as MockExampleKind)
            }
            disabled={!running}
          >
            {EXAMPLE_KINDS.map((kind) => (
              <option key={kind} value={kind}>
                {kind}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field__label">Condition</span>
          <select
            className="select"
            value={source}
            onChange={(event) =>
              setSource(event.target.value as ConditionSource)
            }
            disabled={!running}
          >
            <option value="query">query</option>
            <option value="header">header</option>
            <option value="body">body</option>
          </select>
        </label>
        <label className="field">
          <span className="field__label">
            {source === "body" ? "Body path" : "Name"}
          </span>
          <input
            type="text"
            value={field}
            onChange={(event) => setField(event.target.value)}
            disabled={!running}
            spellCheck={false}
            placeholder={source === "body" ? "items.0.sku" : "status"}
          />
        </label>
        <label className="field">
          <span className="field__label">Equals</span>
          <input
            type="text"
            value={equals}
            onChange={(event) => setEquals(event.target.value)}
            disabled={!running}
            spellCheck={false}
          />
        </label>
      </div>
      <div className="row-actions">
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={addRule}
          disabled={!running || !selectedRoute || !field.trim()}
        >
          <Icon name="plus" size={12} />
          <span>Add rule</span>
        </button>
      </div>

      {entries.length === 0 ? (
        <p className="hint">
          No conditional examples. Add one when a route should switch mock
          kind based on a query value, request header, or JSON body path.
        </p>
      ) : (
        <div className="condition-rules">
          {entries.map(({ route, rules }) => (
            <div key={route} className="condition-rules__group">
              <code className="condition-rules__route">{route}</code>
              <ol className="condition-rules__list">
                {rules.map((rule, ruleIndex) => (
                  <li
                    key={`${route}:${ruleIndex}`}
                    className="condition-rules__rule"
                  >
                    <div className="condition-rules__head">
                      <label className="field">
                        <span className="field__label">Name</span>
                        <input
                          type="text"
                          value={rule.name}
                          onChange={(event) =>
                            updateRule(route, ruleIndex, {
                              name: event.target.value
                            })
                          }
                          disabled={!running}
                          spellCheck={false}
                        />
                      </label>
                      <label className="field">
                        <span className="field__label">Example</span>
                        <select
                          className="select"
                          value={rule.example}
                          onChange={(event) =>
                            updateRule(route, ruleIndex, {
                              example: event.target.value as MockExampleKind
                            })
                          }
                          disabled={!running}
                        >
                          {EXAMPLE_KINDS.map((kind) => (
                            <option key={kind} value={kind}>
                              {kind}
                            </option>
                          ))}
                        </select>
                      </label>
                      <div className="condition-rules__buttons">
                        <button
                          type="button"
                          className="btn btn--icon"
                          onClick={() => moveRule(route, ruleIndex, -1)}
                          title="Move rule up"
                          aria-label={`Move ${rule.name} up`}
                          disabled={!running || ruleIndex === 0}
                        >
                          ^
                        </button>
                        <button
                          type="button"
                          className="btn btn--icon"
                          onClick={() => moveRule(route, ruleIndex, 1)}
                          title="Move rule down"
                          aria-label={`Move ${rule.name} down`}
                          disabled={!running || ruleIndex === rules.length - 1}
                        >
                          v
                        </button>
                        <button
                          type="button"
                          className="btn btn--icon"
                          onClick={() => removeRule(route, ruleIndex)}
                          title="Remove rule"
                          aria-label={`Remove ${rule.name}`}
                          disabled={!running}
                        >
                          <Icon name="close" size={12} />
                        </button>
                      </div>
                    </div>

                    <ul className="condition-rules__conditions">
                      {rule.when.map((condition, conditionIndex) => {
                        const editable = toEditableCondition(condition);
                        return (
                          <li
                            key={`${route}:${ruleIndex}:${conditionIndex}`}
                            className="condition-rules__condition"
                          >
                            <select
                              className="select"
                              value={editable.source}
                              onChange={(event) =>
                                updateCondition(
                                  route,
                                  ruleIndex,
                                  conditionIndex,
                                  {
                                    source: event.target
                                      .value as ConditionSource
                                  }
                                )
                              }
                              disabled={!running}
                              aria-label={`Condition source for ${rule.name}`}
                            >
                              <option value="query">query</option>
                              <option value="header">header</option>
                              <option value="body">body</option>
                            </select>
                            <input
                              type="text"
                              value={editable.field}
                              onChange={(event) =>
                                updateCondition(
                                  route,
                                  ruleIndex,
                                  conditionIndex,
                                  { field: event.target.value }
                                )
                              }
                              disabled={!running}
                              spellCheck={false}
                              aria-label={`Condition field for ${rule.name}`}
                            />
                            <input
                              type="text"
                              value={editable.equalsText}
                              onChange={(event) =>
                                updateCondition(
                                  route,
                                  ruleIndex,
                                  conditionIndex,
                                  { equalsText: event.target.value }
                                )
                              }
                              disabled={!running}
                              spellCheck={false}
                              aria-label={`Condition value for ${rule.name}`}
                            />
                            <button
                              type="button"
                              className="btn btn--icon"
                              onClick={() =>
                                removeCondition(
                                  route,
                                  ruleIndex,
                                  conditionIndex
                                )
                              }
                              title="Remove condition"
                              aria-label={`Remove condition ${conditionIndex + 1} from ${rule.name}`}
                              disabled={!running}
                            >
                              <Icon name="close" size={12} />
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                    <div className="row-actions">
                      <button
                        type="button"
                        className="btn btn--ghost btn--sm"
                        onClick={() => addCondition(route, ruleIndex)}
                        disabled={!running}
                      >
                        <Icon name="plus" size={12} />
                        <span>Add condition</span>
                      </button>
                    </div>
                  </li>
                ))}
              </ol>
            </div>
          ))}
        </div>
      )}

      <div className="row-actions">
        <button
          type="button"
          className="btn btn--primary btn--sm"
          onClick={() => void apply()}
          disabled={!running || busy || !dirty}
        >
          <Icon name="zap" size={12} />
          <span>{busy ? "Applying…" : "Apply rules"}</span>
        </button>
        {dirty ? (
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={reset}
            disabled={busy}
          >
            Reset
          </button>
        ) : null}
      </div>
    </section>
  );
}

interface EditableCondition {
  source: ConditionSource;
  field: string;
  equalsText: string;
}

function routeKeyOf(route: GatewayRouteSummary): string {
  return `${route.method} ${route.path}`;
}

function makeCondition(
  source: ConditionSource,
  field: string,
  equalsText: string
): RequestCondition {
  const key = field.trim();
  if (source === "body") {
    return { source, path: key, equals: parseConditionValue(equalsText) };
  }
  return { source, name: key, equals: equalsText };
}

function patchCondition(
  condition: RequestCondition,
  patch: Partial<EditableCondition>
): RequestCondition {
  const current = toEditableCondition(condition);
  const next = { ...current, ...patch };
  return makeCondition(next.source, next.field, next.equalsText);
}

function toEditableCondition(condition: RequestCondition): EditableCondition {
  if (condition.source === "body") {
    return {
      source: "body",
      field: condition.path,
      equalsText:
        typeof condition.equals === "string"
          ? condition.equals
          : JSON.stringify(condition.equals)
    };
  }
  return {
    source: condition.source,
    field: condition.name,
    equalsText: condition.equals
  };
}

function parseConditionValue(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed) return "";
  try {
    return JSON.parse(trimmed);
  } catch {
    return value;
  }
}

export function normalizeRules(
  value: Record<string, ConditionalExampleRule[]>
): Record<string, ConditionalExampleRule[]> {
  const out: Record<string, ConditionalExampleRule[]> = {};
  for (const [route, rules] of Object.entries(value ?? {})) {
    const routeKey = route.trim();
    if (!routeKey) continue;
    const normalizedRules = rules
      .map((rule) => ({
        name: rule.name.trim() || "Conditional rule",
        example: rule.example,
        when: rule.when
          .map(normalizeCondition)
          .filter((condition): condition is RequestCondition => Boolean(condition))
      }))
      .filter((rule) => rule.when.length > 0);
    if (normalizedRules.length > 0) {
      out[routeKey] = normalizedRules;
    }
  }
  return out;
}

function normalizeCondition(
  condition: RequestCondition
): RequestCondition | null {
  if (condition.source === "body") {
    const path = condition.path.trim();
    if (!path) return null;
    return { ...condition, path };
  }
  const name = condition.name.trim();
  if (!name) return null;
  return { ...condition, name };
}
