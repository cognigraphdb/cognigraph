import { AutoComplete, Form, Input, InputNumber, Modal, Select } from "antd";
import { useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { buildRelationshipRequest, vertexIdError } from "../lib/graph-edges.ts";
import type { ExplorerNode } from "../lib/graph-explorer.ts";
import type { JsonObject, Notify } from "../types.ts";
import { ErrorAlert } from "./ErrorAlert.tsx";

// Rendered conditionally by the parent (mount = open, unmount = closed) —
// the app-wide Modal convention: toggling `open` on a mounted Modal leaves
// the close transition hanging in the Bun dev bundle (see DocumentDialog).
interface CreateEdgeDialogProps {
  api: CogniGraphApi;
  edgeCollections: string[];
  /// Loaded canvas nodes — offered as suggestions for both endpoints.
  nodes: ExplorerNode[];
  initialFrom?: string;
  initialCollection?: string;
  notify: Notify;
  onClose: () => void;
  /// Fires with the from-vertex and the edge collection written to, so the
  /// screen can refresh the neighborhood over the right collection.
  onCreated: (from: string, collection: string) => void;
}

interface Fields {
  from: string;
  to: string;
  collection: string;
  relationType: string;
  confidence: number;
}

export function CreateEdgeDialog({
  api,
  edgeCollections,
  nodes,
  initialFrom,
  initialCollection,
  notify,
  onClose,
  onCreated,
}: CreateEdgeDialogProps) {
  const [form] = Form.useForm<Fields>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const suggestions = nodes.map((node) => ({
    value: node.id,
    label: `${node.id} — ${String(node.vertex.title ?? node.vertex.name ?? "")}`.trim(),
  }));
  const vertexRule = {
    validator: (_: unknown, value: string) => {
      const message = vertexIdError(value ?? "");
      return message ? Promise.reject(new Error(message)) : Promise.resolve();
    },
  };

  async function submit(values: Fields) {
    setBusy(true);
    setError("");
    try {
      await api.post<JsonObject>(
        "/graph/relationships",
        buildRelationshipRequest({
          collection: values.collection,
          from: values.from,
          to: values.to,
          relationType: values.relationType,
          confidence: values.confidence,
        }),
      );
      notify(`Linked ${values.from.trim()} → ${values.to.trim()}`);
      onCreated(values.from.trim(), values.collection);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Relationship creation failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      confirmLoading={busy}
      okText="Create relationship"
      onCancel={onClose}
      onOk={() => form.submit()}
      open
      title="Create a relationship"
    >
      <p className="dialog-hint">
        Links two documents with a typed, weighted edge. Both endpoints are vertex addresses
        (collection/key); pick a canvas node or paste an id.
      </p>
      <Form<Fields>
        form={form}
        initialValues={{
          from: initialFrom ?? "",
          collection: initialCollection ?? edgeCollections[0],
          confidence: 0.9,
        }}
        layout="vertical"
        onFinish={(values) => void submit(values)}
        requiredMark={false}
      >
        <Form.Item label="From" name="from" rules={[vertexRule]}>
          <AutoComplete
            filterOption={(input, option) =>
              String(option?.value ?? "")
                .toLowerCase()
                .includes(input.toLowerCase())
            }
            options={suggestions}
            placeholder="labels/000ae256…"
          />
        </Form.Item>
        <Form.Item label="To" name="to" rules={[vertexRule]}>
          <AutoComplete
            filterOption={(input, option) =>
              String(option?.value ?? "")
                .toLowerCase()
                .includes(input.toLowerCase())
            }
            options={suggestions}
            placeholder="labels/02ab1006…"
          />
        </Form.Item>
        <Form.Item
          label="Edge collection"
          name="collection"
          rules={[{ required: true, message: "Pick an edge collection." }]}
        >
          <Select
            disabled={edgeCollections.length === 0}
            options={edgeCollections.map((name) => ({ label: name, value: name }))}
            placeholder={edgeCollections.length === 0 ? "None in this tenant" : "Select"}
          />
        </Form.Item>
        <Form.Item
          label="Relation type"
          name="relationType"
          rules={[{ required: true, whitespace: true, message: "Name the relation." }]}
        >
          <Input autoComplete="off" placeholder="same_class, cites, supersedes…" />
        </Form.Item>
        <Form.Item label="Confidence" name="confidence">
          <InputNumber controls={false} max={1} min={0} step={0.05} style={{ width: "100%" }} />
        </Form.Item>
      </Form>
      {edgeCollections.length === 0 ? (
        <ErrorAlert title="This tenant has no edge collections — create one on the Collections page first." />
      ) : null}
      {error ? <ErrorAlert title={error} /> : null}
    </Modal>
  );
}
