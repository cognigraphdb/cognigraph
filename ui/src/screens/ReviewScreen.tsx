import { ArrowsClockwise, CircleNotch, Flag, PlusCircle, Scales } from "@phosphor-icons/react";
import {
  Button,
  Result,
  Segmented,
  Select,
  Spin,
  Table,
  type TableColumnsType,
  Tag,
  Tooltip,
} from "antd";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { NeuronInspector, type NeuronVerdict } from "../components/NeuronInspector.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { ProposeNeuronDialog } from "../components/ProposeNeuronDialog.tsx";
import {
  factOf,
  GRADUATION_REASON_META,
  type GraduationCandidate,
  type GraduationResponse,
  KIND_META,
  NEURON_STATUSES,
  type NeuronDoc,
  type NeuronListResponse,
  type NeuronStatus,
  STATUS_META,
} from "../lib/neurons.ts";
import type { JsonObject, Notify } from "../types.ts";

type StatusFilter = NeuronStatus | "all";

const PAGE_LIMIT = 200;

export function ReviewScreen({ api, notify }: { api: CogniGraphApi; notify: Notify }) {
  const [spaces, setSpaces] = useState<string[]>([]);
  const [space, setSpace] = useState<string>();
  const [status, setStatus] = useState<StatusFilter>("proposed");
  const [neurons, setNeurons] = useState<NeuronDoc[]>([]);
  const [flags, setFlags] = useState<GraduationCandidate[]>([]);
  const [selectedKey, setSelectedKey] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [verdictBusy, setVerdictBusy] = useState(false);
  const [proposeOpen, setProposeOpen] = useState(false);
  const [error, setError] = useState("");

  // Space types are ordinary documents — the review queue is scoped to one.
  const loadSpaces = useCallback(async () => {
    try {
      const response = await api.get<{ results: JsonObject[] }>(
        "/documents?collection=space_types&limit=100",
      );
      const ids = (response.results ?? [])
        .map((doc) => String(doc._key ?? ""))
        .filter(Boolean)
        .sort();
      setSpaces(ids);
      setSpace((current) => current ?? ids[0]);
    } catch {
      setSpaces([]);
    }
  }, [api]);

  const loadNeurons = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const params = new URLSearchParams();
      if (space) params.set("space_type", space);
      if (status !== "all") params.set("status", status);
      params.set("limit", String(PAGE_LIMIT));
      const response = await api.get<NeuronListResponse>(`/neurons?${params}`);
      setNeurons(response.neurons ?? []);
    } catch (reason) {
      setNeurons([]);
      setError(reason instanceof Error ? reason.message : "Unable to list neurons");
    } finally {
      setLoading(false);
    }
  }, [api, space, status]);

  // Graduation flags are computed over the space's ACCEPTED neurons; the
  // report is advisory — retirement stays a human verdict in the inspector.
  const loadFlags = useCallback(async () => {
    if (!space) {
      setFlags([]);
      return;
    }
    try {
      const response = await api.get<GraduationResponse>(
        `/neurons/graduation?space_type=${encodeURIComponent(space)}`,
      );
      setFlags(response.candidates ?? []);
    } catch {
      // A missing/invalid space yields a validation error; flags are
      // advisory, so degrade to none rather than blocking the queue.
      setFlags([]);
    }
  }, [api, space]);

  useEffect(() => void loadSpaces(), [loadSpaces]);
  useEffect(() => void loadNeurons(), [loadNeurons]);
  useEffect(() => void loadFlags(), [loadFlags]);

  const flaggedIds = useMemo(() => new Set(flags.map((flag) => flag.neuron_id)), [flags]);
  const selected = neurons.find((neuron) => neuron._key === selectedKey);

  const refresh = () => {
    void loadNeurons();
    void loadFlags();
  };

  async function applyVerdict(neuron: NeuronDoc, verdict: NeuronVerdict, note: string) {
    setVerdictBusy(true);
    try {
      await api.post(
        `/neurons/${encodeURIComponent(neuron._key)}/${verdict}`,
        note ? { note } : {},
      );
      const past = { accept: "accepted", reject: "rejected", retire: "retired" }[verdict];
      notify(`${neuron.id}: ${past}`, "success");
      setSelectedKey(undefined);
      refresh();
    } catch (reason) {
      // Accept re-validates against the stored accepted set — a
      // hint/blocker conflict lands here with the server's explanation.
      notify(reason instanceof Error ? reason.message : "Transition failed", "error");
    } finally {
      setVerdictBusy(false);
    }
  }

  const columns: TableColumnsType<NeuronDoc> = [
    {
      title: "Fact",
      key: "fact",
      render: (_, neuron) => (
        <span className="neuron-fact-cell mono-cell">
          {flaggedIds.has(neuron.id) ? (
            <Tooltip title="Flagged by the graduation report — see Flags above">
              <Flag aria-hidden="true" className="grad-flag" size={14} weight="fill" />
            </Tooltip>
          ) : null}
          <strong>{factOf(neuron)}</strong>
        </span>
      ),
    },
    {
      title: "Kind",
      dataIndex: "type",
      width: 130,
      render: (value: NeuronDoc["type"]) => <Tag>{KIND_META[value]?.label ?? String(value)}</Tag>,
    },
    {
      title: "Status",
      dataIndex: "status",
      width: 110,
      render: (value: NeuronStatus) => (
        <Tag color={STATUS_META[value]?.color}>{STATUS_META[value]?.label ?? String(value)}</Tag>
      ),
    },
    {
      title: "Confidence",
      dataIndex: "confidence",
      width: 110,
      render: (value: number) => value.toFixed(2),
    },
    {
      title: "Proposed by",
      dataIndex: "proposed_by",
      width: 130,
      render: (value) => String(value ?? "—"),
    },
    {
      title: "Reviewed by",
      dataIndex: "reviewed_by",
      width: 130,
      render: (value) => String(value ?? "—"),
    },
  ];

  return (
    <main className={selected ? "workspace with-inspector" : "workspace"}>
      <section className="collection-panel review-panel">
        <PageHeader
          actions={
            <>
              <Button
                disabled={spaces.length === 0}
                icon={<PlusCircle size={17} />}
                onClick={() => setProposeOpen(true)}
                type="primary"
              >
                Propose neuron
              </Button>
              <Button
                disabled={loading}
                icon={
                  loading ? (
                    <CircleNotch className="cg-spin" weight="bold" />
                  ) : (
                    <ArrowsClockwise size={17} />
                  )
                }
                onClick={refresh}
              >
                Refresh
              </Button>
            </>
          }
          description="Review proposed rules, accept or reject them with attribution, and retire neurons the graduation report flags as redundant."
          eyebrow="Governance / review"
          title="Review"
        />

        <div className="workspace-controls">
          <Select
            aria-label="Space type"
            onChange={(value) => {
              setSpace(value);
              setSelectedKey(undefined);
            }}
            options={spaces.map((id) => ({ label: id, value: id }))}
            placeholder="Space type"
            style={{ minWidth: 180 }}
            value={space}
          />
          <Segmented<StatusFilter>
            onChange={(value) => {
              setStatus(value);
              setSelectedKey(undefined);
            }}
            options={[
              ...NEURON_STATUSES.map((value) => ({
                label: STATUS_META[value].label,
                value: value as StatusFilter,
              })),
              { label: "All", value: "all" as StatusFilter },
            ]}
            value={status}
          />
          <span className="workspace-count">
            {neurons.length} {neurons.length === 1 ? "neuron" : "neurons"}
          </span>
        </div>

        {flags.length > 0 ? (
          <section className="console-card grad-card">
            <div className="card-heading compact-heading">
              <h2>
                <Flag aria-hidden="true" size={15} weight="fill" /> Graduation flags
              </h2>
              <span>
                {flags.length} {flags.length === 1 ? "candidate" : "candidates"} — advisory only
              </span>
            </div>
            <ul className="grad-list">
              {flags.map((flag) => (
                <li key={`${flag.neuron_id}-${flag.reason}`}>
                  <button
                    className="grad-target"
                    onClick={() => {
                      setStatus("accepted");
                      setSelectedKey(flag.neuron_id);
                    }}
                    type="button"
                  >
                    {flag.neuron_id}
                  </button>
                  <span className="grad-fact mono-cell">{flag.fact}</span>
                  <span className="grad-reason">{GRADUATION_REASON_META[flag.reason]}</span>
                </li>
              ))}
            </ul>
          </section>
        ) : null}

        <Spin indicator={<CircleNotch className="cg-spin" weight="bold" />} spinning={loading}>
          {!loading && spaces.length === 0 ? (
            <Result
              icon={<Scales aria-hidden="true" size={34} />}
              status="info"
              subTitle={
                <>
                  Review is scoped to a space type, and this server has none yet.
                  <br />
                  <small>
                    Create a document in the <code>space_types</code> collection (entities +
                    relation rules) to open a review queue.
                  </small>
                </>
              }
              title="No spaces to review"
            />
          ) : error ? (
            <Result status="warning" subTitle={error} title="Unable to load the queue" />
          ) : (
            <Table<NeuronDoc>
              columns={columns}
              dataSource={neurons}
              locale={{
                emptyText:
                  status === "proposed"
                    ? "Review queue is empty — nothing awaiting a verdict"
                    : "No neurons match this filter",
              }}
              onRow={(neuron) => ({
                onClick: () => setSelectedKey(neuron._key),
              })}
              pagination={false}
              rowClassName={(neuron) =>
                neuron._key === selectedKey ? "review-row selected" : "review-row"
              }
              rowKey="_key"
              size="small"
            />
          )}
        </Spin>
      </section>

      {selected ? (
        <NeuronInspector
          busy={verdictBusy}
          key={selected._key}
          neuron={selected}
          onClose={() => setSelectedKey(undefined)}
          onVerdict={(verdict, note) => void applyVerdict(selected, verdict, note)}
        />
      ) : null}

      {proposeOpen ? (
        <ProposeNeuronDialog
          api={api}
          defaultSpace={space}
          notify={notify}
          onClose={() => setProposeOpen(false)}
          onProposed={() => {
            setProposeOpen(false);
            setStatus("proposed");
            refresh();
          }}
          spaces={spaces}
        />
      ) : null}
    </main>
  );
}
