import {
  CheckCircle,
  CircleNotch,
  Copy,
  FloppyDisk,
  Lightning,
  PencilSimple,
  Trash,
  X,
} from "@phosphor-icons/react";
import { Button, Input, Tabs, Tooltip } from "antd";
import { useEffect, useState } from "react";
import { embeddingSourceText } from "../lib/api-documents.ts";
import { type DocumentEdit, editedDocument } from "../lib/document-edit.ts";
import { documentJson } from "../lib/documents.ts";
import type { GraphDocument, InspectorTab } from "../types.ts";
import { ErrorAlert } from "./ErrorAlert.tsx";
import { JsonCode } from "./JsonCode.tsx";

interface DocumentInspectorProps {
  document: GraphDocument;
  onClose: () => void;
  onDelete: () => void;
  onUpdate: (edit: DocumentEdit) => Promise<void>;
  /// Embeds the document server-side; resolves when the refreshed
  /// document has replaced this one.
  onEmbed: () => Promise<void>;
}

export function DocumentInspector({
  document,
  onClose,
  onDelete,
  onUpdate,
  onEmbed,
}: DocumentInspectorProps) {
  const [tab, setTab] = useState<InspectorTab>("json");
  const [editing, setEditing] = useState(false);
  const [editBase, setEditBase] = useState(document);
  const [draft, setDraft] = useState(documentJson(document));
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const [embedding, setEmbedding] = useState(false);
  const [saving, setSaving] = useState(false);
  const embeddable = embeddingSourceText(document).length > 0;

  const embed = async () => {
    setEmbedding(true);
    try {
      await onEmbed();
    } finally {
      setEmbedding(false);
    }
  };

  useEffect(() => {
    if (!editing) {
      setDraft(documentJson(document));
      setError("");
    }
  }, [document, editing]);

  const copyKey = async () => {
    await navigator.clipboard.writeText(document._key);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  };

  const save = async () => {
    let updated: DocumentEdit;
    try {
      updated = editedDocument(editBase, draft);
    } catch (cause) {
      setError(cause instanceof SyntaxError ? "The document is not valid JSON." : String(cause));
      return;
    }
    setSaving(true);
    try {
      await onUpdate(updated);
      setEditing(false);
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Document update failed.");
    } finally {
      setSaving(false);
    }
  };

  const jsonContent = !editing ? (
    <JsonCode value={documentJson(document)} />
  ) : (
    <div className="editor-wrap">
      <p className="editor-guidance">
        Save updates changed fields. To clear a value, use null; field removal is unsupported. _id,
        _key, _rev, created_at and updated_at are read-only.
      </p>
      <Input.TextArea
        aria-label="Document JSON"
        onChange={(event) => setDraft(event.target.value)}
        spellCheck={false}
        value={draft}
      />
      {error ? <ErrorAlert title={error} /> : null}
    </div>
  );

  return (
    <aside className="inspector">
      <header className="inspector-header">
        <div className="key-block">
          <span>KEY</span>
          <div>
            <strong>{document._key}</strong>
            <Tooltip title={copied ? "Copied" : "Copy document key"}>
              <Button
                aria-label="Copy document key"
                icon={
                  copied ? (
                    <CheckCircle aria-hidden="true" size={17} />
                  ) : (
                    <Copy aria-hidden="true" size={17} />
                  )
                }
                onClick={copyKey}
                size="small"
                type="text"
              />
            </Tooltip>
          </div>
        </div>
        <div className="inspector-actions">
          {editing ? (
            <>
              <Button
                disabled={saving}
                icon={<FloppyDisk aria-hidden="true" size={17} />}
                onClick={() => void save()}
              >
                {saving ? "Saving…" : "Save"}
              </Button>
              <Button disabled={saving} onClick={() => setEditing(false)}>
                Cancel
              </Button>
            </>
          ) : (
            <Button
              icon={<PencilSimple aria-hidden="true" size={17} />}
              onClick={() => {
                setEditBase(document);
                setDraft(documentJson(document));
                setEditing(true);
              }}
            >
              Edit
            </Button>
          )}
          <Button
            danger
            disabled={saving}
            icon={<Trash aria-hidden="true" size={17} />}
            onClick={onDelete}
          >
            Delete
          </Button>
          <Button
            aria-label="Close inspector"
            disabled={saving}
            icon={<X aria-hidden="true" size={20} />}
            onClick={onClose}
            type="text"
          />
        </div>
      </header>

      <Tabs
        activeKey={tab}
        className="inspector-tabs"
        items={[
          {
            key: "json",
            label: "JSON",
            children: <div className="inspector-content">{jsonContent}</div>,
          },
          {
            key: "metadata",
            label: "Metadata",
            children: (
              <div className="inspector-content">
                <Metadata document={document} />
              </div>
            ),
          },
        ]}
        onChange={(key) => setTab(key as InspectorTab)}
      />

      <footer className="inspector-footer">
        <span className={`embedding-status ${document.embedding}`}>
          <CheckCircle aria-hidden="true" size={15} /> Embedding: {document.embedding}
        </span>
        {document.model && <span>Model: {document.model}</span>}
        {document.dimensions && <span>Dimensions: {document.dimensions}</span>}
        <Tooltip
          title={
            embeddable
              ? "Embeds the document's title, summary, and text server-side"
              : "This document has no embeddable text"
          }
        >
          <Button
            className="embed-button"
            disabled={embedding || editing || !embeddable}
            icon={
              embedding ? (
                <CircleNotch className="cg-spin" weight="bold" />
              ) : (
                <Lightning aria-hidden="true" size={15} />
              )
            }
            onClick={() => void embed()}
            size="small"
          >
            {embedding
              ? "Embedding…"
              : document.embedding === "ready"
                ? "Re-embed"
                : "Generate embedding"}
          </Button>
        </Tooltip>
      </footer>
    </aside>
  );
}

function Metadata({ document }: { document: GraphDocument }) {
  return (
    <dl className="metadata-list">
      <div>
        <dt>Collection</dt>
        <dd>{document._id.split("/")[0]}</dd>
      </div>
      <div>
        <dt>Identifier</dt>
        <dd>{document._id}</dd>
      </div>
      <div>
        <dt>Created</dt>
        <dd>{document.createdAt}</dd>
      </div>
      <div>
        <dt>Updated</dt>
        <dd>{document.updatedAt}</dd>
      </div>
      <div>
        <dt>Owner</dt>
        <dd>{document.owner}</dd>
      </div>
      <div>
        <dt>Tags</dt>
        <dd>{document.tags.join(", ")}</dd>
      </div>
    </dl>
  );
}
