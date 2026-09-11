import { Form, Input, InputNumber, Modal, Select } from "antd";
import { useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { KIND_META, type NeuronKind } from "../lib/neurons.ts";
import type { Notify } from "../types.ts";
import { ErrorAlert } from "./ErrorAlert.tsx";

// Rendered conditionally by the parent (mount = open, unmount = closed) —
// the app-wide Modal convention: toggling `open` on a mounted Modal leaves
// the close transition hanging in the Bun dev bundle (see DocumentDialog).
interface ProposeNeuronDialogProps {
  api: CogniGraphApi;
  spaces: string[];
  defaultSpace?: string;
  notify: Notify;
  onClose: () => void;
  onProposed: () => void;
}

interface Fields {
  space_type: string;
  id: string;
  kind: NeuronKind;
  confidence: number;
  rationale?: string;
  evidence?: string;
  // alias:
  entity?: string;
  aliases?: string;
  // relation hint / blocker / rank hint:
  source?: string;
  relation?: string;
  target?: string;
  triggers?: string;
  boost?: number;
}

const splitLines = (text: string | undefined): string[] =>
  (text ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);

/// Author a rule proposal. The server validates it against the space's
/// ontology and stores it as `proposed` no matter what — acceptance is a
/// separate, separately-validated transition on the Review screen.
export function ProposeNeuronDialog({
  api,
  spaces,
  defaultSpace,
  notify,
  onClose,
  onProposed,
}: ProposeNeuronDialogProps) {
  const [form] = Form.useForm<Fields>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const kind = Form.useWatch("kind", form) ?? "relation_hint";

  async function submit(values: Fields) {
    setBusy(true);
    setError("");
    const payload: Record<string, unknown> = {
      space_type: values.space_type,
      id: values.id.trim(),
      type: values.kind,
      confidence: values.confidence,
      rationale: values.rationale?.trim() ?? "",
      evidence: splitLines(values.evidence),
    };
    if (values.kind === "alias") {
      payload.entity = values.entity?.trim() ?? "";
      payload.aliases = splitLines(values.aliases);
    } else {
      payload.relation = values.relation?.trim() ?? "";
      if (values.kind === "relation_rank_hint") {
        payload.boost = values.boost ?? 0;
      } else {
        payload.source = values.source?.trim() ?? "";
        payload.target = values.target?.trim() ?? "";
        payload.triggers = splitLines(values.triggers);
      }
    }
    try {
      await api.post("/neurons", payload);
      notify(`Proposed ${values.id.trim()}`);
      onProposed();
    } catch (reason) {
      // Ontology violations surface here (unknown entity, bad relation…) —
      // show the server's words, they name the offending field.
      setError(reason instanceof Error ? reason.message : "Proposal failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      confirmLoading={busy}
      okText="Propose"
      onCancel={onClose}
      onOk={() => form.submit()}
      open
      title="Propose a neuron"
    >
      <p className="dialog-hint">
        Proposals are validated against the space ontology and enter the review queue as{" "}
        <strong>proposed</strong>. Only accepted neurons influence construction.
      </p>
      <Form<Fields>
        form={form}
        initialValues={{
          space_type: defaultSpace ?? spaces[0],
          kind: "relation_hint",
          confidence: 0.9,
        }}
        layout="vertical"
        onFinish={(values) => void submit(values)}
        requiredMark={false}
      >
        <Form.Item label="Space type" name="space_type" rules={[{ required: true }]}>
          <Select options={spaces.map((space) => ({ label: space, value: space }))} />
        </Form.Item>
        <Form.Item
          label="Neuron id"
          name="id"
          rules={[{ required: true, message: "Give the rule a stable id." }]}
        >
          <Input placeholder="supply-hint-meridian" />
        </Form.Item>
        <Form.Item label="Kind" name="kind" rules={[{ required: true }]}>
          <Select
            options={(Object.keys(KIND_META) as NeuronKind[]).map((value) => ({
              label: `${KIND_META[value].label} — ${KIND_META[value].hint}`,
              value,
            }))}
          />
        </Form.Item>

        {kind === "alias" ? (
          <>
            <Form.Item label="Entity" name="entity" rules={[{ required: true }]}>
              <Input placeholder="Meridian" />
            </Form.Item>
            <Form.Item label="Aliases (one per line)" name="aliases" rules={[{ required: true }]}>
              <Input.TextArea
                autoSize={{ minRows: 2, maxRows: 4 }}
                placeholder={"MRD\nMeridian AG"}
              />
            </Form.Item>
          </>
        ) : (
          <>
            {kind !== "relation_rank_hint" ? (
              <Form.Item label="Source entity" name="source" rules={[{ required: true }]}>
                <Input placeholder="Meridian" />
              </Form.Item>
            ) : null}
            <Form.Item label="Relation" name="relation" rules={[{ required: true }]}>
              <Input placeholder="SUPPLIES" />
            </Form.Item>
            {kind !== "relation_rank_hint" ? (
              <>
                <Form.Item label="Target entity" name="target" rules={[{ required: true }]}>
                  <Input placeholder="Compound X" />
                </Form.Item>
                <Form.Item
                  label={
                    kind === "relation_blocker"
                      ? "Veto phrases (one per line)"
                      : "Triggers (one per line)"
                  }
                  name="triggers"
                  rules={[{ required: true }]}
                >
                  <Input.TextArea
                    autoSize={{ minRows: 2, maxRows: 4 }}
                    placeholder="meridian supplies compound x"
                  />
                </Form.Item>
              </>
            ) : (
              <Form.Item
                label="Boost (negative demotes)"
                name="boost"
                rules={[{ required: true, message: "Boost must be a non-zero number." }]}
              >
                <InputNumber controls={false} step={0.1} style={{ width: "100%" }} />
              </Form.Item>
            )}
          </>
        )}

        <Form.Item label="Confidence" name="confidence" rules={[{ required: true }]}>
          <InputNumber controls={false} max={1} min={0} step={0.05} style={{ width: "100%" }} />
        </Form.Item>
        <Form.Item label="Rationale" name="rationale">
          <Input.TextArea
            autoSize={{ minRows: 1, maxRows: 3 }}
            placeholder="Why this rule exists"
          />
        </Form.Item>
        {/* The server rejects evidence-free neurons for every kind — the
            rule is evidence-bound by design, so require it up front. */}
        <Form.Item
          label="Evidence (one quote per line)"
          name="evidence"
          rules={[{ required: true, message: "Quote the source text this rule is grounded in." }]}
        >
          <Input.TextArea
            autoSize={{ minRows: 1, maxRows: 4 }}
            placeholder="Quoted source text supporting the rule"
          />
        </Form.Item>
      </Form>
      {error ? <ErrorAlert title={error} /> : null}
    </Modal>
  );
}
