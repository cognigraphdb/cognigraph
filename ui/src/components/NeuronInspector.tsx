import {
  CheckCircle,
  CircleNotch,
  Prohibit,
  Quotes,
  WarningCircle,
  X,
  XCircle,
} from "@phosphor-icons/react";
import { Button, Input, Popconfirm, Tabs, Tag, Tooltip } from "antd";
import { useState } from "react";
import {
  factOf,
  formatUnixSeconds,
  KIND_META,
  type NeuronDoc,
  STATUS_META,
  triggersOf,
  verdictsFor,
} from "../lib/neurons.ts";
import { JsonCode } from "./JsonCode.tsx";

export type NeuronVerdict = "accept" | "reject" | "retire";

interface NeuronInspectorProps {
  neuron: NeuronDoc;
  busy: boolean;
  onVerdict: (verdict: NeuronVerdict, note: string) => void;
  onClose: () => void;
}

const VERDICT_META: Record<
  NeuronVerdict,
  { label: string; confirm: string; icon: typeof CheckCircle; danger: boolean }
> = {
  accept: {
    label: "Accept",
    confirm: "Accept this neuron? It will start influencing construction.",
    icon: CheckCircle,
    danger: false,
  },
  reject: {
    label: "Reject",
    confirm: "Reject this proposal? It stays stored with your attribution.",
    icon: XCircle,
    danger: true,
  },
  retire: {
    label: "Retire",
    confirm: "Retire this neuron? It stops influencing construction.",
    icon: Prohibit,
    danger: true,
  },
};

// Rendered with key={neuron._key} by the caller: a new selection is a new
// review context, so the draft note and tab reset by remounting.
export function NeuronInspector({ neuron, busy, onVerdict, onClose }: NeuronInspectorProps) {
  const [note, setNote] = useState("");
  const [tab, setTab] = useState<"detail" | "json">("detail");

  const verdicts = verdictsFor(neuron.status);
  const triggers = triggersOf(neuron);
  const status = STATUS_META[neuron.status] ?? { label: neuron.status, color: "default" };
  const kind = KIND_META[neuron.type] ?? { label: neuron.type, hint: "" };

  const detail = (
    <div className="neuron-detail">
      <div className="chip-base neuron-fact mono-cell">{factOf(neuron)}</div>

      <dl className="neuron-props">
        <dt>Space</dt>
        <dd>{neuron.space_type}</dd>
        <dt>Confidence</dt>
        <dd>{neuron.confidence}</dd>
        {neuron.type === "relation_rank_hint" ? (
          <>
            <dt>Boost</dt>
            <dd>{neuron.boost}</dd>
          </>
        ) : null}
      </dl>

      {neuron.rationale ? (
        <section className="neuron-block">
          <h3>Rationale</h3>
          <p>{neuron.rationale}</p>
        </section>
      ) : null}

      {neuron.evidence.length > 0 ? (
        <section className="neuron-block">
          <h3>Evidence</h3>
          <ul className="neuron-quotes">
            {neuron.evidence.map((quote) => (
              <li key={quote}>
                <Quotes aria-hidden="true" className="quote-icon" size={13} weight="fill" />
                <span>{quote}</span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {triggers.length > 0 ? (
        <section className="neuron-block">
          <h3>{neuron.type === "relation_blocker" ? "Veto phrases" : "Grounding triggers"}</h3>
          <div className="neuron-triggers">
            {triggers.map((trigger) => (
              <code className="chip-base" key={trigger}>
                {trigger}
              </code>
            ))}
          </div>
        </section>
      ) : null}

      <section className="neuron-block">
        <h3>Audit trail</h3>
        <dl className="neuron-props">
          <dt>Proposed by</dt>
          <dd>
            {neuron.proposed_by ?? "—"}
            <small> {formatUnixSeconds(neuron.proposed_at)}</small>
          </dd>
          <dt>Reviewed by</dt>
          <dd>
            {neuron.reviewed_by ?? "—"}
            <small> {formatUnixSeconds(neuron.reviewed_at)}</small>
          </dd>
          {neuron.review_note ? (
            <>
              <dt>Review note</dt>
              <dd>{neuron.review_note}</dd>
            </>
          ) : null}
        </dl>
      </section>
    </div>
  );

  return (
    <aside className="inspector neuron-inspector">
      <header className="inspector-header">
        <div className="key-block">
          <span>NEURON</span>
          <strong className="neuron-id">{neuron.id}</strong>
          <div className="neuron-tags">
            <Tooltip title={kind.hint}>
              <Tag>{kind.label}</Tag>
            </Tooltip>
            <Tag color={status.color}>{status.label}</Tag>
          </div>
        </div>
        <Button aria-label="Close inspector" icon={<X size={16} />} onClick={onClose} type="text" />
      </header>

      <div className="inspector-body">
        <Tabs
          activeKey={tab}
          items={[
            { key: "detail", label: "Review", children: detail },
            {
              key: "json",
              label: "JSON",
              children: <JsonCode value={JSON.stringify(neuron, null, 2)} />,
            },
          ]}
          onChange={(key) => setTab(key as typeof tab)}
          size="small"
        />
      </div>

      {verdicts.length > 0 ? (
        <footer className="neuron-verdicts">
          <Input.TextArea
            autoSize={{ minRows: 1, maxRows: 3 }}
            disabled={busy}
            onChange={(event) => setNote(event.target.value)}
            placeholder="Review note (stored with your attribution)"
            value={note}
          />
          <div className="actions-row-base verdict-buttons">
            {verdicts.map((verdict) => {
              const meta = VERDICT_META[verdict];
              const Icon = meta.icon;
              return (
                <Popconfirm
                  cancelText="Cancel"
                  icon={<WarningCircle className="confirm-icon" size={17} weight="fill" />}
                  key={verdict}
                  okText={meta.label}
                  onConfirm={() => onVerdict(verdict, note.trim())}
                  placement="topLeft"
                  title={meta.confirm}
                >
                  {/* icon swap instead of the antd `loading` prop:
                      `icon` + `loading` together route the custom icon
                      through @ant-design/icons and trip its dev warning. */}
                  <Button
                    danger={meta.danger}
                    disabled={busy}
                    icon={
                      busy ? (
                        <CircleNotch className="cg-spin" weight="bold" />
                      ) : (
                        <Icon size={16} weight="bold" />
                      )
                    }
                    type={verdict === "accept" ? "primary" : "default"}
                  >
                    {meta.label}
                  </Button>
                </Popconfirm>
              );
            })}
          </div>
        </footer>
      ) : null}
    </aside>
  );
}
