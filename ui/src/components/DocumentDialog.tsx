import { CircleNotch, Trash } from "@phosphor-icons/react";
import { Form, Input, Modal, Select } from "antd";
import { useEffect, useState } from "react";
import type { DocumentDraft } from "../types.ts";

interface DocumentDialogProps {
  mode: "create" | "delete";
  /// The collection the action applies to (named in the dialog copy).
  collection: string;
  title?: string;
  onCancel: () => void;
  onCreate?: (draft: DocumentDraft) => Promise<void> | void;
  onDelete?: () => Promise<void> | void;
}

const emptyDraft: DocumentDraft = { title: "", category: "documentation", summary: "" };

export function DocumentDialog(props: DocumentDialogProps) {
  const [form] = Form.useForm<DocumentDraft>();
  const [submitting, setSubmitting] = useState(false);
  // Capture before the conditionally mounted Modal moves focus inside itself.
  // Its unmount skips the normal close animation's focus-restoration callback.
  const [opener] = useState(() => document.activeElement);
  useEffect(
    () => () => {
      requestAnimationFrame(() => {
        if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
      });
    },
    [opener],
  );

  const run = async (action: (() => Promise<void> | void) | undefined) => {
    if (!action) return;
    setSubmitting(true);
    try {
      await action();
    } finally {
      setSubmitting(false);
    }
  };

  if (props.mode === "delete") {
    return (
      <Modal
        centered
        okButtonProps={{ danger: true, disabled: submitting }}
        okText={
          submitting ? (
            <span className="dialog-busy-label">
              <CircleNotch className="cg-spin" /> Deleting…
            </span>
          ) : (
            "Delete document"
          )
        }
        onCancel={props.onCancel}
        onOk={() => void run(props.onDelete)}
        open
        title="Delete document?"
      >
        <div className="delete-dialog-copy">
          <span className="dialog-icon danger">
            <Trash aria-hidden="true" size={22} />
          </span>
          <p>
            <strong>{props.title}</strong> will be removed from the{" "}
            <strong>{props.collection}</strong> collection. This action cannot be undone.
          </p>
        </div>
      </Modal>
    );
  }

  const submit = () => {
    void form
      .validateFields()
      .then((draft) => run(() => props.onCreate?.(draft)))
      .catch(() => undefined);
  };

  return (
    <Modal
      centered
      okButtonProps={{ disabled: submitting }}
      okText={
        submitting ? (
          <span className="dialog-busy-label">
            <CircleNotch className="cg-spin" /> Creating…
          </span>
        ) : (
          "Create document"
        )
      }
      onCancel={props.onCancel}
      onOk={submit}
      open
      title="Create document"
    >
      <p className="dialog-description">
        Add a JSON document to the <strong>{props.collection}</strong> collection.
      </p>
      <Form<DocumentDraft>
        form={form}
        initialValues={emptyDraft}
        layout="vertical"
        requiredMark="optional"
      >
        <Form.Item
          label="Title"
          name="title"
          rules={[
            { required: true, message: "Enter a document title" },
            { max: 160, message: "Keep the title under 160 characters" },
          ]}
        >
          <Input autoFocus placeholder="Document title" />
        </Form.Item>
        <Form.Item label="Category" name="category" rules={[{ required: true }]}>
          <Select
            options={["documentation", "strategy", "research", "policy"].map((value) => ({
              label: value,
              value,
            }))}
          />
        </Form.Item>
        <Form.Item
          label="Summary"
          name="summary"
          rules={[{ max: 500, message: "Keep the summary under 500 characters" }]}
        >
          <Input.TextArea placeholder="What does this document contain?" rows={4} showCount />
        </Form.Item>
      </Form>
    </Modal>
  );
}
