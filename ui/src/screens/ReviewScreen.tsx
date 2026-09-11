import { ArrowsClockwise, CircleNotch, Flag, PlusCircle, Scales } from "@phosphor-icons/react";
import {
  Alert,
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
import { useSearchParams } from "react-router";
import type { CogniGraphApi } from "../api/client.ts";
import { useAccess } from "../components/AccessBoundary.tsx";
import { NeuronInspector, type NeuronVerdict } from "../components/NeuronInspector.tsx";
import { PageHeader } from "../components/PageHeader.tsx";
import { ProposeNeuronDialog } from "../components/ProposeNeuronDialog.tsx";
import { useReviewData } from "../hooks/useReviewData.ts";
import { useSpaceTypes } from "../hooks/useSpaceTypes.ts";
import {
  factOf,
  GRADUATION_REASON_META,
  KIND_META,
  NEURON_STATUSES,
  type NeuronDoc,
  type NeuronStatus,
  STATUS_META,
} from "../lib/neurons.ts";
import { neuronPageLabel, reviewLocation, type StatusFilter } from "../lib/review-data.ts";
import type { Notify } from "../types.ts";

export function ReviewScreen({ api, notify }: { api: CogniGraphApi; notify: Notify }) {
  const { reviewWrite } = useAccess();
  const catalog = useSpaceTypes(api);
  const { spaces } = catalog;
  const [params, setParams] = useSearchParams();
  const location = reviewLocation(params);
  const space = location.space ?? spaces[0];
  const { status, page, selectedKey } = location;
  const data = useReviewData(api, space, status, page, selectedKey);
  const { selected, flags, error } = data;
  const neurons = data.queue?.neurons ?? [];
  const loading = catalog.loading || data.loading;
  const [verdictBusy, setVerdictBusy] = useState(false);
  const [proposeOpen, setProposeOpen] = useState(false);
  const updateLocation = useCallback(
    (values: Record<string, string | undefined>) => {
      setParams((current) => {
        const next = new URLSearchParams(current);
        for (const [key, value] of Object.entries(values)) {
          if (value === undefined) next.delete(key);
          else next.set(key, value);
        }
        return next;
      });
    },
    [setParams],
  );
  const actualPage = data.queue?.page;
  useEffect(() => {
    if (actualPage !== undefined && actualPage !== page) {
      setParams(
        (current) => {
          const next = new URLSearchParams(current);
          next.set("page", String(actualPage));
          return next;
        },
        { replace: true },
      );
    }
  }, [actualPage, page, setParams]);
  const flaggedIds = useMemo(() => new Set(flags.map((flag) => flag.neuron_id)), [flags]);
  const refresh = () => {
    catalog.refresh();
    data.refresh();
  };

  async function applyVerdict(neuron: NeuronDoc, verdict: NeuronVerdict, note: string) {
    if (!reviewWrite || verdictBusy || data.selectionLoading) return;
    setVerdictBusy(true);
    try {
      await api.post(
        `/neurons/${encodeURIComponent(neuron._key)}/${verdict}`,
        note ? { note } : {},
      );
      const past = { accept: "accepted", reject: "rejected", retire: "retired" }[verdict];
      notify(`${neuron.id}: ${past}`, "success");
      // Keep the selected identity even when its new status removes it from the
      // current filter. Reload its stored state and the shifted page separately.
      data.refresh();
    } catch (reason) {
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
          <button
            aria-label={`Inspect neuron ${neuron.id}: ${factOf(neuron)}`}
            aria-pressed={neuron._key === selectedKey}
            className="table-action"
            disabled={verdictBusy}
            onClick={(event) => {
              event.stopPropagation();
              updateLocation({ space, neuron: neuron._key });
            }}
            type="button"
          >
            <strong>{factOf(neuron)}</strong>
          </button>
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
                disabled={
                  !reviewWrite ||
                  verdictBusy ||
                  catalog.loading ||
                  Boolean(catalog.error) ||
                  spaces.length === 0
                }
                icon={<PlusCircle size={17} />}
                title={
                  !reviewWrite
                    ? "Your role can read neurons but cannot propose or review them."
                    : undefined
                }
                onClick={() => setProposeOpen(true)}
                type="primary"
              >
                Propose neuron
              </Button>
              <Button
                disabled={loading || verdictBusy}
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
          description={
            reviewWrite
              ? "Review proposed rules, accept or reject them with attribution, and retire neurons the graduation report flags as redundant."
              : "Read-only access to neurons and graduation flags. Your role cannot propose, accept, reject or retire neurons."
          }
          eyebrow="Governance / review"
          title="Review"
        />

        <div className="workspace-controls">
          <Select
            aria-label="Space type"
            disabled={catalog.loading || verdictBusy}
            loading={catalog.loading}
            showSearch={{ optionFilterProp: "label" }}
            onChange={(value) => updateLocation({ space: value, page: "1", neuron: undefined })}
            options={spaces.map((id) => ({ label: id, value: id }))}
            placeholder="Space type"
            style={{ minWidth: 180 }}
            value={space}
          />
          <Segmented<StatusFilter>
            disabled={verdictBusy}
            onChange={(value) =>
              updateLocation({ space, status: value, page: "1", neuron: undefined })
            }
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
            {catalog.loading
              ? "Loading spaces…"
              : catalog.error
                ? "Space catalog unavailable"
                : `${spaces.length} spaces`}
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
                    disabled={verdictBusy}
                    onClick={() =>
                      updateLocation({
                        space,
                        status: "accepted",
                        page: "1",
                        neuron: flag.neuron_id,
                      })
                    }
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

        <div className="workspace-controls review-paging">
          <Button
            aria-label="Previous neuron page"
            disabled={loading || verdictBusy || page <= 1}
            onClick={() => updateLocation({ space, page: String(page - 1) })}
          >
            Previous
          </Button>
          <span>Page {data.queue?.page ?? page}</span>
          <Button
            aria-label="Next neuron page"
            disabled={loading || verdictBusy || !data.queue?.hasMore}
            onClick={() => updateLocation({ space, page: String(page + 1) })}
          >
            Next
          </Button>
          <span className="workspace-count" aria-live="polite">
            {loading
              ? "Loading queue…"
              : error
                ? "Queue unavailable"
                : data.queue
                  ? neuronPageLabel(data.queue)
                  : "Select a space"}
          </span>
        </div>
        <div className="review-notices">
          {data.flagsError ? (
            <Alert type="warning" title={`Graduation flags unavailable: ${data.flagsError}`} />
          ) : null}
          {data.selectionError ? (
            <Alert type="error" title={`Selected neuron unavailable: ${data.selectionError}`} />
          ) : null}
          {data.selectionLoading ? <Alert type="info" title="Loading selected neuron…" /> : null}
          {selected && data.queue && !neurons.some((row) => row._key === selected._key) ? (
            <Alert
              type="info"
              title="The selected neuron is outside this page or filter. Its current state is shown in the inspector."
            />
          ) : null}
        </div>
        <Spin indicator={<CircleNotch className="cg-spin" weight="bold" />} spinning={loading}>
          {catalog.error ? (
            <Result status="warning" title="Unable to load spaces" subTitle={catalog.error} />
          ) : !loading && spaces.length === 0 ? (
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
                onClick: () => {
                  if (!verdictBusy) updateLocation({ space, neuron: neuron._key });
                },
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
          busy={verdictBusy || data.selectionLoading}
          key={selected._key}
          neuron={selected}
          onClose={() => updateLocation({ neuron: undefined })}
          onVerdict={(verdict, note) => void applyVerdict(selected, verdict, note)}
        />
      ) : null}

      {proposeOpen && reviewWrite ? (
        <ProposeNeuronDialog
          api={api}
          defaultSpace={space}
          notify={notify}
          onClose={() => setProposeOpen(false)}
          onProposed={(proposedSpace, key) => {
            setProposeOpen(false);
            updateLocation({ space: proposedSpace, status: "proposed", page: "1", neuron: key });
            refresh();
          }}
          spaces={spaces}
        />
      ) : null}
    </main>
  );
}
