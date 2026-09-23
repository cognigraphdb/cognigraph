//! `cognigraph` — administration CLI for a running CogniGraph server.
//! Connection via --url/--token or COGNIGRAPH_URL/COGNIGRAPH_TOKEN.

mod arangodump;
mod args;
mod client;
mod references;

use args::{Command, USAGE, parse};
use client::{Api, read_password};
use serde_json::{Value, json};

#[cfg(any(test, feature = "enterprise"))]
const PROMOTION_STATUS_PATH: &str = "/admin/promotions/status";
#[cfg(any(test, feature = "enterprise"))]
const PROMOTION_RECONCILE_PATH: &str = "/admin/promotions/reconcile";
#[cfg(any(test, feature = "enterprise"))]
const PROMOTION_RECOVER_PATH: &str = "/admin/promotions/recover";

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let invocation = match parse(&raw) {
        Ok(invocation) => invocation,
        Err(message) => {
            eprintln!("error: {message}");
            std::process::exit(2);
        }
    };
    if invocation.command == Command::Help {
        print!("{USAGE}");
        return;
    }
    // Offline: no server, URL or token.
    if let Command::ImportArangodump(options) = &invocation.command {
        std::process::exit(arangodump::run_cli(options));
    }
    let url = invocation
        .url
        .or_else(|| {
            std::env::var("COGNIGRAPH_URL")
                .ok()
                .filter(|v| !v.is_empty())
        })
        .unwrap_or_else(|| "http://127.0.0.1:3000".into());
    let token = invocation.token.or_else(|| {
        std::env::var("COGNIGRAPH_TOKEN")
            .ok()
            .filter(|v| !v.is_empty())
    });
    let api = Api::new(url, token);

    match run(&api, invocation.command) {
        Ok(Some(output)) => println!("{}", serde_json::to_string_pretty(&output).unwrap()),
        Ok(None) => {}
        Err(message) => {
            eprintln!("error: {message}");
            std::process::exit(1);
        }
    }
}

fn run(api: &Api, command: Command) -> Result<Option<Value>, String> {
    match command {
        Command::Help | Command::ImportArangodump(_) => unreachable!("handled in main"),
        Command::ReferencesAudit { snapshot } => references::audit_file(&snapshot).map(Some),
        Command::ReferencesRepair {
            snapshot,
            plan,
            out,
        } => {
            let (repaired, report) = references::repair_files(&snapshot, &plan)?;
            write_new_private_file(&out, &repaired)?;
            Ok(Some(report))
        }
        Command::Health => {
            let liveness = api.get_root("/health")?;
            let database = api.get_root("/health/database")?;
            Ok(Some(json!({ "health": liveness, "database": database })))
        }
        Command::Export { out } => {
            let snapshot = api.get("/admin/export")?;
            match out {
                Some(path) => {
                    let body = serde_json::to_string_pretty(&snapshot).unwrap();
                    std::fs::write(&path, body).map_err(|e| format!("write {path}: {e}"))?;
                    eprintln!("snapshot written to {path}");
                    Ok(None)
                }
                None => Ok(Some(snapshot)),
            }
        }
        Command::Import { file } => {
            let raw = match file {
                Some(path) => {
                    std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))?
                }
                None => std::io::read_to_string(std::io::stdin())
                    .map_err(|e| format!("read stdin: {e}"))?,
            };
            let snapshot: Value =
                serde_json::from_str(&raw).map_err(|e| format!("snapshot is not JSON: {e}"))?;
            Ok(Some(api.post("/admin/import", &snapshot)?))
        }
        Command::Query {
            query,
            write,
            binds,
        } => {
            let bind_vars: serde_json::Map<String, Value> = binds.into_iter().collect();
            let body = json!({ "query": query, "bind_vars": bind_vars });
            let path = if write { "/query" } else { "/search/query" };
            Ok(Some(api.post(path, &body)?))
        }
        Command::DocGet { collection, key } => {
            Ok(Some(api.get(&format!("/documents/{collection}/{key}"))?))
        }
        Command::DocList {
            collection,
            limit,
            offset,
        } => {
            let mut path = format!("/documents?collection={collection}");
            if let Some(limit) = limit {
                path.push_str(&format!("&limit={limit}"));
            }
            if let Some(offset) = offset {
                path.push_str(&format!("&offset={offset}"));
            }
            Ok(Some(api.get(&path)?))
        }
        Command::DocPut {
            collection,
            document,
        } => {
            let raw = match document.strip_prefix('@') {
                Some(path) => {
                    std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?
                }
                None => document,
            };
            let mut body: Value =
                serde_json::from_str(&raw).map_err(|e| format!("document is not JSON: {e}"))?;
            let Some(fields) = body.as_object_mut() else {
                return Err("document must be a JSON object".into());
            };
            fields.insert("collection".into(), json!(collection));
            Ok(Some(api.post("/documents", &body)?))
        }
        Command::DocDelete { collection, key } => {
            Ok(Some(api.delete(&format!("/documents/{collection}/{key}"))?))
        }
        Command::IndexList { collection } => Ok(Some(
            api.get(&format!("/collections/{collection}/indexes"))?,
        )),
        Command::IndexEnsure {
            collection,
            fields,
            name,
            unique,
            sparse,
        } => {
            let mut body = json!({ "fields": fields, "unique": unique, "sparse": sparse });
            if let Some(name) = name {
                body["name"] = json!(name);
            }
            Ok(Some(api.post(
                &format!("/collections/{collection}/indexes"),
                &body,
            )?))
        }
        Command::IndexDrop { collection, name } => Ok(Some(
            api.delete(&format!("/collections/{collection}/indexes/{name}"))?,
        )),
        Command::UserCreate {
            username,
            role,
            tenant,
        } => {
            let password = read_password()?;
            let mut body = json!({ "username": username, "password": password, "role": role });
            if let Some(tenant) = tenant {
                body["tenant"] = json!(tenant);
            }
            Ok(Some(api.post("/users", &body)?))
        }
        Command::UserList => Ok(Some(api.get("/users")?)),
        Command::UserDelete { key } => {
            let key = resolve_user_key(api, &key)?;
            Ok(Some(api.delete(&format!("/users/{key}"))?))
        }
        Command::TokenIssue { user, ttl_secs } => {
            let key = resolve_user_key(api, &user)?;
            let mut body = json!({});
            if let Some(ttl) = ttl_secs {
                body["expires_in_secs"] = json!(ttl);
            }
            Ok(Some(api.post(&format!("/users/{key}/tokens"), &body)?))
        }
        Command::TokenList { user } => {
            let key = resolve_user_key(api, &user)?;
            Ok(Some(api.get(&format!("/users/{key}/tokens"))?))
        }
        Command::TokenRevoke { user, token } => {
            let key = resolve_user_key(api, &user)?;
            Ok(Some(api.delete(&format!("/users/{key}/tokens/{token}"))?))
        }
        Command::Embed {
            collection,
            file,
            text_field,
            batch_size,
        } => {
            let raw = std::fs::read_to_string(&file).map_err(|e| format!("read {file}: {e}"))?;
            let items: Vec<Value> = raw
                .lines()
                .filter(|line| !line.trim().is_empty())
                .enumerate()
                .map(|(i, line)| {
                    serde_json::from_str(line).map_err(|e| format!("{file} line {}: {e}", i + 1))
                })
                .collect::<Result<_, _>>()?;
            let mut body = json!({ "collection": collection, "items": items });
            if let Some(field) = text_field {
                body["text_field"] = json!(field);
            }
            if let Some(size) = batch_size {
                body["batch_size"] = json!(size);
            }
            Ok(Some(api.post("/documents/embed", &body)?))
        }
        Command::TokenRotate {
            user,
            token,
            ttl_secs,
        } => {
            let key = resolve_user_key(api, &user)?;
            let mut body = json!({});
            if let Some(ttl) = ttl_secs {
                body["expires_in_secs"] = json!(ttl);
            }
            Ok(Some(api.post(
                &format!("/users/{key}/tokens/{token}/rotate"),
                &body,
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::NeuronList { space, status } => {
            let mut path = "/neurons?".to_string();
            if let Some(space) = space {
                path.push_str(&format!("space_type={space}&"));
            }
            if let Some(status) = status {
                path.push_str(&format!("status={status}"));
            }
            Ok(Some(api.get(path.trim_end_matches(['&', '?']))?))
        }
        #[cfg(feature = "enterprise")]
        Command::NeuronPropose { document } => {
            let raw = match document.strip_prefix('@') {
                Some(path) => {
                    std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?
                }
                None => document,
            };
            let body: Value =
                serde_json::from_str(&raw).map_err(|e| format!("neuron is not JSON: {e}"))?;
            Ok(Some(api.post("/neurons", &body)?))
        }
        #[cfg(feature = "enterprise")]
        Command::NeuronTransition { key, action, note } => {
            let mut body = json!({});
            if let Some(note) = note {
                body["note"] = json!(note);
            }
            Ok(Some(api.post(&format!("/neurons/{key}/{action}"), &body)?))
        }
        #[cfg(feature = "enterprise")]
        Command::NeuronGraduation { space } => Ok(Some(
            api.get(&format!("/neurons/graduation?space_type={space}"))?,
        )),
        #[cfg(feature = "enterprise")]
        Command::NeuronReview { space } => Ok(Some(
            api.post("/construct/review", &json!({ "space_type": space }))?,
        )),
        #[cfg(feature = "enterprise")]
        Command::Advise { space } => Ok(Some(
            api.post("/construct/advise", &json!({ "space_type": space }))?,
        )),
        #[cfg(feature = "enterprise")]
        Command::Draft { space, file } => {
            let raw = std::fs::read_to_string(&file).map_err(|e| format!("read {file}: {e}"))?;
            let chunks: Vec<Value> = raw
                .lines()
                .filter(|line| !line.trim().is_empty())
                .enumerate()
                .map(|(i, line)| {
                    serde_json::from_str(line).map_err(|e| format!("{file} line {}: {e}", i + 1))
                })
                .collect::<Result<_, _>>()?;
            Ok(Some(api.post(
                "/construct/draft",
                &json!({ "space_type": space, "chunks": chunks }),
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::DraftAccept { space } => Ok(Some(
            api.post(&format!("/construct/draft/{space}/accept"), &json!({}))?,
        )),
        #[cfg(feature = "enterprise")]
        Command::TenantList => Ok(Some(api.get("/tenants")?)),
        #[cfg(feature = "enterprise")]
        Command::TenantCreate { name } => Ok(Some(api.post("/tenants", &json!({ "name": name }))?)),
        #[cfg(feature = "enterprise")]
        Command::TenantBootstrapAdmin { name, username } => {
            let password = read_password()?;
            Ok(Some(api.post(
                &format!("/tenants/{name}/admin"),
                &json!({ "username": username, "password": password }),
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::TenantStatus { name, status } => Ok(Some(
            api.post(&format!("/tenants/{name}"), &json!({ "status": status }))?,
        )),
        #[cfg(feature = "enterprise")]
        Command::TenantDelete { name } => Ok(Some(api.delete(&format!("/tenants/{name}"))?)),
        #[cfg(feature = "enterprise")]
        Command::JobSubmit {
            kind,
            input,
            idempotency_key,
        } => {
            let input = read_json_argument(&input, "job input")?;
            Ok(Some(api.post_idempotent(
                "/jobs",
                &json!({ "kind": kind, "input": input }),
                &idempotency_key,
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::JobList {
            kind,
            status,
            limit,
            cursor,
            archived,
            offset,
        } => {
            let path = job_list_path(
                kind.as_deref(),
                status.as_deref(),
                limit,
                cursor.as_deref(),
                archived.as_deref(),
                offset,
            );
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::JobQueueStatus => Ok(Some(api.get("/admin/jobs/status")?)),
        #[cfg(feature = "enterprise")]
        Command::JobReconcile {
            dry_run,
            limit,
            cursor,
        } => Ok(Some(api.post(
            "/admin/jobs/reconcile",
            &job_reconcile_body(dry_run, limit, cursor),
        )?)),
        #[cfg(feature = "enterprise")]
        Command::JobArchive {
            before_ms,
            limit,
            dry_run,
            reason,
        } => Ok(Some(api.post(
            "/admin/jobs/archive",
            &job_archive_body(before_ms, limit, dry_run, reason),
        )?)),
        #[cfg(feature = "enterprise")]
        Command::JobStatus { id } => Ok(Some(api.get(&format!("/jobs/{id}"))?)),
        #[cfg(feature = "enterprise")]
        Command::JobCancel { id, reason } => {
            let mut body = json!({});
            if let Some(reason) = reason {
                body["reason"] = json!(reason);
            }
            Ok(Some(api.post(&format!("/jobs/{id}/cancel"), &body)?))
        }
        #[cfg(feature = "enterprise")]
        Command::JobRetry {
            id,
            idempotency_key,
            mode,
            reason,
        } => {
            let mut body = json!({});
            if let Some(mode) = mode {
                body["mode"] = json!(mode);
            }
            if let Some(reason) = reason {
                body["reason"] = json!(reason);
            }
            Ok(Some(api.post_idempotent(
                &format!("/jobs/{id}/retry"),
                &body,
                &idempotency_key,
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionEvidenceCreate {
            evidence,
            idempotency_key,
        } => {
            let body = read_json_argument(&evidence, "promotion evidence")?;
            let path = encoded_api_path(&["promotions", "evidence"], &[]);
            Ok(Some(api.post_idempotent(&path, &body, &idempotency_key)?))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionEvidenceList { limit, cursor } => {
            let path = promotion_page_path(&["promotions", "evidence"], limit, cursor.as_deref());
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionEvidenceShow { id } => {
            let path = encoded_api_path(&["promotions", "evidence", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionPromote {
            evidence_id,
            intent,
            idempotency_key,
        } => {
            let path = encoded_api_path(&["promotions", "evidence", &evidence_id, "promote"], &[]);
            let body = read_public_governance_json_argument(&intent, "signed promotion intent")?;
            Ok(Some(api.post_idempotent(&path, &body, &idempotency_key)?))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionReject {
            evidence_id,
            intent,
            idempotency_key,
        } => {
            let path = encoded_api_path(&["promotions", "evidence", &evidence_id, "reject"], &[]);
            let body = read_public_governance_json_argument(&intent, "signed promotion intent")?;
            Ok(Some(api.post_idempotent(&path, &body, &idempotency_key)?))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionDecisions { limit, cursor } => {
            let path = promotion_page_path(&["promotions", "decisions"], limit, cursor.as_deref());
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionDecisionShow { id } => {
            let path = encoded_api_path(&["promotions", "decisions", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionCurrent {
            space_type,
            channel,
        } => {
            let path = encoded_api_path(&["promotions", "current", &space_type, &channel], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionRollback {
            space_type,
            channel,
            intent,
            idempotency_key,
        } => {
            let path = encoded_api_path(
                &["promotions", "current", &space_type, &channel, "rollback"],
                &[],
            );
            let body = read_public_governance_json_argument(&intent, "signed promotion intent")?;
            Ok(Some(api.post_idempotent(&path, &body, &idempotency_key)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceStatus => Ok(Some(api.get("/governance/status")?)),
        #[cfg(feature = "enterprise")]
        Command::GovernanceKeyList { limit, cursor } => {
            let path = promotion_page_path(&["governance", "keys"], limit, cursor.as_deref());
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceKeyRegister {
            request,
            idempotency_key,
        } => {
            let body = read_public_governance_json_argument(
                &request,
                "signed governance key registration",
            )?;
            Ok(Some(api.post_idempotent(
                "/governance/keys",
                &body,
                &idempotency_key,
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceKeyShow { id } => {
            let path = encoded_api_path(&["governance", "keys", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceKeyRevoke {
            registration_id,
            request,
            idempotency_key,
        } => {
            let path = encoded_api_path(&["governance", "keys", &registration_id, "revoke"], &[]);
            let body =
                read_public_governance_json_argument(&request, "signed governance key revocation")?;
            Ok(Some(api.post_idempotent(&path, &body, &idempotency_key)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceRevocationList { limit, cursor } => {
            let path =
                promotion_page_path(&["governance", "revocations"], limit, cursor.as_deref());
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceRevocationShow { id } => {
            let path = encoded_api_path(&["governance", "revocations", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernancePolicyList { limit, cursor } => {
            let path = promotion_page_path(&["governance", "policies"], limit, cursor.as_deref());
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernancePolicyCreate {
            request,
            idempotency_key,
        } => {
            let body = read_public_governance_json_argument(&request, "signed policy revision")?;
            Ok(Some(api.post_idempotent(
                "/governance/policies",
                &body,
                &idempotency_key,
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernancePolicyShow { id } => {
            let path = encoded_api_path(&["governance", "policies", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernancePolicyApprove {
            id,
            request,
            idempotency_key,
        } => {
            let path = encoded_api_path(&["governance", "policies", &id, "approve"], &[]);
            let body = read_public_governance_json_argument(&request, "signed policy approval")?;
            Ok(Some(api.post_idempotent(&path, &body, &idempotency_key)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceApprovalShow { id } => {
            let path = encoded_api_path(&["governance", "approvals", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceBindingResolve { approval_id } => {
            let path = encoded_api_path(&["governance", "bindings", &approval_id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceArtifactAttest {
            request,
            idempotency_key,
        } => {
            let body =
                read_public_governance_json_argument(&request, "signed artifact attestation")?;
            Ok(Some(api.post_idempotent(
                "/governance/artifact-attestations",
                &body,
                &idempotency_key,
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceArtifactList { limit, cursor } => {
            let path = promotion_page_path(
                &["governance", "artifact-attestations"],
                limit,
                cursor.as_deref(),
            );
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceArtifactShow { id } => {
            let path = encoded_api_path(&["governance", "artifact-attestations", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::GovernanceArtifactBindingResolve { request } => {
            let body = read_public_governance_json_argument(&request, "artifact binding request")?;
            Ok(Some(
                api.post("/governance/artifact-bindings/resolve", &body)?,
            ))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairRevisionSubmit {
            request,
            idempotency_key,
        } => {
            let body =
                read_public_governance_json_argument(&request, "signed semantic repair revision")?;
            Ok(Some(api.post_idempotent(
                "/semantic-repairs/revisions",
                &body,
                &idempotency_key,
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairRevisionList { limit, cursor } => {
            let path =
                promotion_page_path(&["semantic-repairs", "revisions"], limit, cursor.as_deref());
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairRevisionShow { id } => {
            let path = encoded_api_path(&["semantic-repairs", "revisions", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairReviewSubmit {
            revision_id,
            request,
            idempotency_key,
        } => {
            let body =
                read_public_governance_json_argument(&request, "signed semantic repair review")?;
            let path = encoded_api_path(
                &["semantic-repairs", "revisions", &revision_id, "review"],
                &[],
            );
            Ok(Some(api.post_idempotent(&path, &body, &idempotency_key)?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairReviewList { limit, cursor } => {
            let path =
                promotion_page_path(&["semantic-repairs", "reviews"], limit, cursor.as_deref());
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairReviewShow { id } => {
            let path = encoded_api_path(&["semantic-repairs", "reviews", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairCurrent {
            space_type,
            channel,
        } => {
            let path =
                encoded_api_path(&["semantic-repairs", "current", &space_type, &channel], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairGenerationBuild {
            target_space,
            channel,
            expected_promotion_head_decision_id,
            idempotency_key,
        } => {
            let body = semantic_repair_generation_build_body(
                &target_space,
                &channel,
                &expected_promotion_head_decision_id,
            );
            Ok(Some(api.post_idempotent(
                "/semantic-repairs/generations",
                &body,
                &idempotency_key,
            )?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairGenerationList { limit, cursor } => {
            let path = promotion_page_path(
                &["semantic-repairs", "generations"],
                limit,
                cursor.as_deref(),
            );
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairGenerationShow { id } => {
            let path = encoded_api_path(&["semantic-repairs", "generations", &id], &[]);
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairGenerationDeploy {
            id,
            request,
            idempotency_key,
        } => {
            let body = read_public_governance_json_argument(
                &request,
                "signed semantic repair deployment intent",
            )?;
            let path = encoded_api_path(&["semantic-repairs", "generations", &id, "deploy"], &[]);
            Ok(Some(api.post_idempotent(&path, &body, &idempotency_key)?))
        }
        #[cfg(feature = "enterprise")]
        Command::SemanticRepairDeploymentCurrent { space_type } => {
            let path = encoded_api_path(
                &["semantic-repairs", "deployments", "current", &space_type],
                &[],
            );
            Ok(Some(api.get(&path)?))
        }
        #[cfg(feature = "enterprise")]
        Command::ArtifactCustodyPlan { evidence_id, out } => {
            let plan = fetch_artifact_recovery_plan(api, &evidence_id)?;
            if let Some(path) = out {
                write_new_private_file(&path, &plan.canonical_bytes().map_err(|e| e.to_string())?)?;
                eprintln!("artifact recovery plan written to {path}");
                Ok(None)
            } else {
                Ok(Some(serde_json::to_value(plan).map_err(|e| e.to_string())?))
            }
        }
        #[cfg(feature = "enterprise")]
        Command::ArtifactCustodyCreate {
            evidence_id,
            cas_root,
            out,
            max_bytes,
        } => {
            let max_bytes = max_bytes.unwrap_or(cognigraph_artifacts::DEFAULT_MAX_CUSTODY_BYTES);
            let plan = fetch_artifact_recovery_plan(api, &evidence_id)?;
            let source = cognigraph_artifacts::LocalArtifactCas::open(&cas_root, max_bytes)
                .map_err(|e| e.to_string())?;
            let receipt = cognigraph_artifacts::create_bundle(&source, &plan, &out, max_bytes)
                .map_err(|e| e.to_string())?;
            Ok(Some(json!({
                "bundle": out,
                "plan_digest": plan.plan_digest,
                "backup_receipt": receipt,
            })))
        }
        #[cfg(feature = "enterprise")]
        Command::ArtifactCustodyVerify {
            bundle,
            expected_plan_digest,
            tenant,
            incarnation,
            max_bytes,
        } => {
            let receipt = cognigraph_artifacts::verify_bundle(
                &bundle,
                &expected_plan_digest,
                &tenant,
                &incarnation,
                max_bytes.unwrap_or(cognigraph_artifacts::DEFAULT_MAX_CUSTODY_BYTES),
            )
            .map_err(|e| e.to_string())?;
            Ok(Some(json!({
                "bundle": bundle,
                "verified": true,
                "backup_receipt": receipt,
            })))
        }
        #[cfg(feature = "enterprise")]
        Command::ArtifactCustodyRestore {
            bundle,
            cas_root,
            expected_plan_digest,
            tenant,
            incarnation,
            receipt,
            max_bytes,
        } => {
            require_new_file_destination(&receipt)?;
            require_output_outside_directory(&receipt, &cas_root, "destination CAS")?;
            require_output_outside_directory(&receipt, &bundle, "artifact recovery bundle")?;
            let restored = cognigraph_artifacts::restore_bundle(
                &bundle,
                &cas_root,
                &expected_plan_digest,
                &tenant,
                &incarnation,
                max_bytes.unwrap_or(cognigraph_artifacts::DEFAULT_MAX_CUSTODY_BYTES),
            )
            .map_err(|e| e.to_string())?;
            if let Err(error) = write_new_private_file(
                &receipt,
                &restored.canonical_bytes().map_err(|e| e.to_string())?,
            ) {
                return Err(format!(
                    "artifact scope was restored and fully verified, but its external receipt could not be persisted: {error}"
                ));
            }
            Ok(Some(json!({
                "bundle": bundle,
                "cas_root": cas_root,
                "receipt": receipt,
                "restored": true,
                "restore_receipt": restored,
            })))
        }
        #[cfg(feature = "enterprise")]
        Command::PromotionStatus => Ok(Some(api.get(PROMOTION_STATUS_PATH)?)),
        #[cfg(feature = "enterprise")]
        Command::PromotionRecover => Ok(Some(api.post(PROMOTION_RECOVER_PATH, &json!({}))?)),
        #[cfg(feature = "enterprise")]
        Command::PromotionReconcile {
            space_type,
            channel,
            dry_run,
        } => Ok(Some(api.post(
            PROMOTION_RECONCILE_PATH,
            &promotion_reconcile_body(&space_type, &channel, dry_run),
        )?)),
        #[cfg(feature = "enterprise")]
        Command::NeuronAudit { space } => {
            let response = api.get(&format!("/neurons?space_type={space}&status=accepted"))?;
            let samples: Vec<Value> = response["neurons"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|n| n["audit_sample"] == json!(true))
                .map(|n| {
                    json!({
                        "key": n["_key"],
                        "fact": format!(
                            "{} --{}--> {}",
                            n["source"].as_str().unwrap_or("?"),
                            n["relation"].as_str().unwrap_or("?"),
                            n["target"].as_str().unwrap_or("?")
                        ),
                        "reviewed_by": n["reviewed_by"],
                        "judge_confidence": n["judge_confidence"],
                        "judge_reasoning": n["judge_reasoning"],
                    })
                })
                .collect();
            Ok(Some(json!({
                "audit_queue": samples.len(),
                "samples": samples,
                "note": "agree = leave; disagree = `neuron retire KEY --note ...` (human-only lane)",
            })))
        }
        #[cfg(feature = "enterprise")]
        Command::NeuronPending { space, limit } => {
            let mut path = "/neurons?status=proposed".to_string();
            if let Some(space) = &space {
                path.push_str(&format!("&space_type={space}"));
            }
            if let Some(limit) = limit {
                path.push_str(&format!("&limit={limit}"));
            }
            let response = api.get(&path)?;
            // Digest: what a reviewer needs at a glance, oldest first.
            let mut queue: Vec<Value> = response["neurons"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|n| {
                    json!({
                        "key": n["_key"],
                        "space_type": n["space_type"],
                        "type": n["type"],
                        "fact": format!(
                            "{} --{}--> {}",
                            n["source"].as_str().unwrap_or("?"),
                            n["relation"].as_str().unwrap_or("?"),
                            n["target"].as_str().unwrap_or("?")
                        ),
                        "triggers": n["triggers"],
                        "proposed_by": n["proposed_by"],
                        "proposed_at": n["proposed_at"],
                        "rationale": n["rationale"],
                    })
                })
                .collect();
            queue.sort_by_key(|n| n["proposed_at"].as_u64().unwrap_or(0));
            Ok(Some(json!({ "pending": queue.len(), "queue": queue })))
        }
        #[cfg(feature = "enterprise")]
        Command::NeuronShow { key } => {
            let neuron = api.get(&format!("/documents/neurons/{key}"))?;
            let space = neuron["space_type"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let triggers: Vec<String> = neuron["triggers"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect();

            // The chunks this neuron's triggers actually match, fetched via
            // CGQL — a reviewer should never need a second tool to see what
            // they are approving. (For blockers the triggers are vetoes:
            // matches show the context that WOULD be suppressed.)
            let mut matches: Vec<Value> = Vec::new();
            let mut unmatched: Vec<String> = Vec::new();
            if !triggers.is_empty() && !space.is_empty() {
                let chunks = api.post(
                    "/search/query",
                    &json!({
                        "query": "FOR c IN chunks FILTER c.space_id == @space                                   RETURN { key: c._key, text: c.text }",
                        "bind_vars": { "space": space },
                    }),
                )?;
                let chunks = chunks["results"].as_array().cloned().unwrap_or_default();
                for trigger in &triggers {
                    let needle = trigger.to_lowercase();
                    let mut hit = false;
                    for chunk in &chunks {
                        let text = chunk["text"].as_str().unwrap_or_default();
                        if text.to_lowercase().contains(&needle) {
                            matches.push(json!({
                                "trigger": trigger,
                                "chunk": chunk["key"],
                                "text": text,
                            }));
                            hit = true;
                        }
                    }
                    if !hit {
                        unmatched.push(trigger.clone());
                    }
                }
            }
            Ok(Some(json!({
                "neuron": neuron,
                "evidence_matches": matches,
                "triggers_matching_no_chunk": unmatched,
            })))
        }
        Command::CacheStats => Ok(Some(api.get("/cache/stats")?)),
        Command::CacheClear => Ok(Some(api.post("/cache/clear", &json!({}))?)),
        Command::Login { username } => {
            let password = read_password()?;
            Ok(Some(api.post(
                "/auth/login",
                &json!({ "username": username, "password": password }),
            )?))
        }
        #[cfg(not(feature = "enterprise"))]
        _ => Err("enterprise_feature_required: use the Enterprise CLI build".into()),
    }
}

#[cfg(feature = "enterprise")]
fn fetch_artifact_recovery_plan(
    api: &Api,
    evidence_id: &str,
) -> Result<cognigraph_artifacts::ArtifactRecoveryPlanV1, String> {
    let path = encoded_api_path(&["admin", "artifact-custody", "evidence", evidence_id], &[]);
    let plan: cognigraph_artifacts::ArtifactRecoveryPlanV1 =
        serde_json::from_value(api.get(&path)?).map_err(|error| {
            format!("server returned an invalid artifact recovery plan: {error}")
        })?;
    plan.validate()
        .map_err(|error| format!("server returned a malformed artifact recovery plan: {error}"))?;
    Ok(plan)
}

fn write_new_private_file(path: &str, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;

    let (parent, destination) = checked_new_file_destination(path)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut stage = None;
    for sequence in 0..16_u8 {
        let candidate = parent.join(format!(
            ".cognigraph-output-{}-{nonce}-{sequence}.partial",
            std::process::id()
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(file) => {
                stage = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "create output stage in {}: {error}",
                    parent.display()
                ));
            }
        }
    }
    let (stage, mut file) =
        stage.ok_or_else(|| "could not allocate a unique output staging file".to_string())?;
    let prepared = (|| {
        file.write_all(bytes)
            .map_err(|error| format!("write output stage {}: {error}", stage.display()))?;
        file.sync_all()
            .map_err(|error| format!("sync output stage {}: {error}", stage.display()))?;
        Ok::<(), String>(())
    })();
    drop(file);
    if let Err(error) = prepared {
        let _ = std::fs::remove_file(&stage);
        return Err(error);
    }

    if let Err(error) = atomic_publish_file_no_replace(&stage, &destination) {
        let _ = std::fs::remove_file(&stage);
        return Err(error);
    }
    if let Err(error) = sync_output_parent(&parent) {
        return Err(format!(
            "output was published at {}, but parent-directory durability is uncertain: {error}",
            destination.display()
        ));
    }
    let observed = std::fs::read(&destination).map_err(|error| {
        format!(
            "output was published at {}, but its final verification failed: {error}",
            destination.display()
        )
    })?;
    if observed != bytes {
        return Err(format!(
            "output was published at {}, but its final bytes changed; inspect that path and do not overwrite it",
            destination.display()
        ));
    }
    Ok(())
}

fn checked_new_file_destination(
    path: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let requested = std::path::Path::new(path);
    let requested_parent = requested
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let metadata = std::fs::symlink_metadata(requested_parent).map_err(|error| {
        format!(
            "inspect output parent {}: {error}",
            requested_parent.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "output parent {} must be a normal non-symlink directory",
            requested_parent.display()
        ));
    }
    let parent = std::fs::canonicalize(requested_parent).map_err(|error| {
        format!(
            "canonicalize output parent {}: {error}",
            requested_parent.display()
        )
    })?;
    let name = requested
        .file_name()
        .ok_or_else(|| "output destination must have a normal file name".to_string())?;
    if name == "." || name == ".." {
        return Err("output destination must have a normal file name".into());
    }
    let destination = parent.join(name);
    match std::fs::symlink_metadata(&destination) {
        Ok(_) => Err(format!(
            "output destination {} already exists; it will not be replaced",
            destination.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((parent, destination)),
        Err(error) => Err(format!(
            "inspect output destination {}: {error}",
            destination.display()
        )),
    }
}

#[cfg(feature = "enterprise")]
fn require_new_file_destination(path: &str) -> Result<(), String> {
    checked_new_file_destination(path).map(|_| ())
}

#[cfg(any(test, feature = "enterprise"))]
fn require_output_outside_directory(
    output: &str,
    directory: &str,
    directory_label: &str,
) -> Result<(), String> {
    let (_, output) = checked_new_file_destination(output)?;
    let directory = std::path::Path::new(directory);
    let metadata = std::fs::symlink_metadata(directory)
        .map_err(|error| format!("inspect {directory_label} {}: {error}", directory.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "{directory_label} {} must be a normal non-symlink directory",
            directory.display()
        ));
    }
    let directory = std::fs::canonicalize(directory).map_err(|error| {
        format!(
            "canonicalize {directory_label} {}: {error}",
            directory.display()
        )
    })?;
    if output.starts_with(&directory) {
        return Err(format!(
            "restore receipt {} must remain outside {directory_label} {}",
            output.display(),
            directory.display()
        ));
    }
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios"
))]
fn atomic_publish_file_no_replace(
    stage: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), String> {
    use rustix::fs::{RenameFlags, renameat_with};

    let parent = stage
        .parent()
        .ok_or_else(|| "output stage has no parent".to_string())?;
    if destination.parent() != Some(parent) {
        return Err("output publication must remain within one parent directory".into());
    }
    let directory = std::fs::File::open(parent)
        .map_err(|error| format!("open output parent {}: {error}", parent.display()))?;
    renameat_with(
        &directory,
        stage
            .file_name()
            .ok_or_else(|| "output stage has no file name".to_string())?,
        &directory,
        destination
            .file_name()
            .ok_or_else(|| "output destination has no file name".to_string())?,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        format!(
            "publish output {} without replacement: {}",
            destination.display(),
            std::io::Error::from_raw_os_error(error.raw_os_error())
        )
    })
}

#[cfg(windows)]
fn atomic_publish_file_no_replace(
    stage: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), String> {
    std::fs::hard_link(stage, destination).map_err(|error| {
        format!(
            "publish output {} without replacement: {error}",
            destination.display()
        )
    })?;
    std::fs::remove_file(stage).map_err(|error| {
        format!(
            "output was published at {}, but its staging link {} could not be removed: {error}",
            destination.display(),
            stage.display()
        )
    })
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    windows
)))]
fn atomic_publish_file_no_replace(
    _stage: &std::path::Path,
    _destination: &std::path::Path,
) -> Result<(), String> {
    Err("atomic no-replace output publication is unsupported on this platform".into())
}

#[cfg(unix)]
fn sync_output_parent(parent: &std::path::Path) -> Result<(), String> {
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("sync output parent {}: {error}", parent.display()))
}

#[cfg(not(unix))]
fn sync_output_parent(_parent: &std::path::Path) -> Result<(), String> {
    Ok(())
}

#[cfg(any(test, feature = "enterprise"))]
fn job_list_path(
    kind: Option<&str>,
    status: Option<&str>,
    limit: Option<u64>,
    cursor: Option<&str>,
    archived: Option<&str>,
    offset: Option<u64>,
) -> String {
    let mut query = Vec::new();
    if let Some(kind) = kind {
        query.push(format!("kind={kind}"));
    }
    if let Some(status) = status {
        query.push(format!("status={status}"));
    }
    if let Some(limit) = limit {
        query.push(format!("limit={limit}"));
    }
    if let Some(cursor) = cursor {
        query.push(format!("cursor={cursor}"));
    }
    if let Some(archived) = archived {
        query.push(format!("archived={archived}"));
    }
    if let Some(offset) = offset {
        query.push(format!("offset={offset}"));
    }
    if query.is_empty() {
        "/jobs".to_string()
    } else {
        format!("/jobs?{}", query.join("&"))
    }
}

#[cfg(any(test, feature = "enterprise"))]
fn job_reconcile_body(dry_run: bool, limit: Option<u64>, cursor: Option<String>) -> Value {
    let mut body = json!({ "dry_run": dry_run });
    if let Some(limit) = limit {
        body["limit"] = json!(limit);
    }
    if let Some(cursor) = cursor {
        body["cursor"] = json!(cursor);
    }
    body
}

#[cfg(any(test, feature = "enterprise"))]
fn job_archive_body(
    before_ms: u64,
    limit: Option<u64>,
    dry_run: bool,
    reason: Option<String>,
) -> Value {
    let mut body = json!({
        "before_ms": before_ms,
        "dry_run": dry_run,
    });
    if let Some(limit) = limit {
        body["limit"] = json!(limit);
    }
    if let Some(reason) = reason {
        body["reason"] = json!(reason);
    }
    body
}

#[cfg(any(test, feature = "enterprise"))]
fn encoded_api_path(segments: &[&str], query: &[(&str, String)]) -> String {
    let mut url = reqwest::Url::parse("http://cognigraph.invalid/")
        .expect("the internal CLI URL base is valid");
    {
        let mut path = url
            .path_segments_mut()
            .expect("the internal CLI URL base supports path segments");
        path.clear();
        for segment in segments {
            path.push(segment);
        }
    }
    if !query.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (name, value) in query {
            pairs.append_pair(name, value);
        }
    }
    let mut path = url.path().to_string();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    path
}

#[cfg(any(test, feature = "enterprise"))]
fn promotion_page_path(segments: &[&str], limit: Option<u64>, cursor: Option<&str>) -> String {
    let mut query = Vec::new();
    if let Some(limit) = limit {
        query.push(("limit", limit.to_string()));
    }
    if let Some(cursor) = cursor {
        query.push(("cursor", cursor.to_string()));
    }
    encoded_api_path(segments, &query)
}

#[cfg(any(test, feature = "enterprise"))]
fn promotion_reconcile_body(space_type: &str, channel: &str, dry_run: bool) -> Value {
    json!({
        "target": {
            "space_type": space_type,
            "channel": channel,
        },
        "dry_run": dry_run,
    })
}

#[cfg(any(test, feature = "enterprise"))]
fn semantic_repair_generation_build_body(
    space_type: &str,
    channel: &str,
    expected_promotion_head_decision_id: &str,
) -> Value {
    json!({
        "target": {
            "space_type": space_type,
            "channel": channel,
        },
        "expected_promotion_head_decision_id": expected_promotion_head_decision_id,
    })
}

#[cfg(any(test, feature = "enterprise"))]
fn read_json_argument(argument: &str, label: &str) -> Result<Value, String> {
    let raw = match argument.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?,
        None => argument.to_string(),
    };
    serde_json::from_str(&raw).map_err(|e| format!("{label} is not JSON: {e}"))
}

/// Governance commands accept only already-signed public envelopes. Refuse
/// common private-key field names locally so operator key material is never
/// included in an HTTP request, even if a key file is selected accidentally.
#[cfg(any(test, feature = "enterprise"))]
fn read_public_governance_json_argument(argument: &str, label: &str) -> Result<Value, String> {
    let value = read_json_argument(argument, label)?;
    if let Some(field) = private_key_field(&value) {
        return Err(format!(
            "{label} contains private key field `{field}`; only pre-signed public statements may be sent"
        ));
    }
    Ok(value)
}

#[cfg(any(test, feature = "enterprise"))]
fn private_key_field(value: &Value) -> Option<&str> {
    match value {
        Value::Object(fields) => {
            for (name, value) in fields {
                let normalized = name.to_ascii_lowercase().replace('-', "_");
                let compact = normalized.replace('_', "");
                if normalized == "secret_key"
                    || normalized == "signing_key"
                    || normalized.contains("private_key")
                    || matches!(compact.as_str(), "secretkey" | "signingkey")
                    || compact.contains("privatekey")
                {
                    return Some(name);
                }
                if let Some(name) = private_key_field(value) {
                    return Some(name);
                }
            }
            None
        }
        Value::Array(values) => values.iter().find_map(private_key_field),
        _ => None,
    }
}

/// User commands accept a username or a raw user key: usernames resolve
/// through the admin user list; anything unmatched passes through as a key.
fn resolve_user_key(api: &Api, user: &str) -> Result<String, String> {
    let listing = api.get("/users")?;
    // The route returns a bare array; tolerate a wrapped shape too.
    let users = listing
        .as_array()
        .or_else(|| listing.get("users").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default();
    for entry in users {
        if entry.get("username").and_then(Value::as_str) == Some(user)
            && let Some(key) = entry.get("key").and_then(Value::as_str)
        {
            return Ok(key.to_string());
        }
    }
    Ok(user.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "enterprise"))]
    #[test]
    fn enterprise_commands_fail_locally_before_network_or_file_access() {
        let api = Api::new("http://127.0.0.1:1".into(), None);
        for command in [
            Command::TenantList,
            Command::NeuronList {
                status: None,
                space: None,
            },
            Command::JobList {
                kind: None,
                status: None,
                limit: None,
                cursor: None,
                archived: None,
                offset: None,
            },
        ] {
            assert!(
                run(&api, command)
                    .unwrap_err()
                    .starts_with("enterprise_feature_required:")
            );
        }
        assert!(!USAGE.contains("tenant create"));
        assert!(!USAGE.contains("neuron list"));
        assert!(USAGE.contains("doc put"));
    }

    #[test]
    fn m17_job_list_query_maps_cursor_archive_and_legacy_offset() {
        assert_eq!(
            job_list_path(
                Some("construct.evaluate"),
                Some("failed"),
                Some(25),
                Some("v1.next_page-2"),
                Some("include"),
                None,
            ),
            "/jobs?kind=construct.evaluate&status=failed&limit=25&cursor=v1.next_page-2&archived=include"
        );
        assert_eq!(
            job_list_path(None, None, None, None, None, Some(10)),
            "/jobs?offset=10"
        );
        assert_eq!(job_list_path(None, None, None, None, None, None), "/jobs");
    }

    #[test]
    fn m17_operator_request_bodies_match_server_contract() {
        assert_eq!(
            job_reconcile_body(true, Some(250), Some("next-1".into())),
            json!({
                "dry_run": true,
                "limit": 250,
                "cursor": "next-1",
            })
        );
        assert_eq!(
            job_archive_body(
                1_780_000_000_000,
                Some(100),
                false,
                Some("retention policy".into()),
            ),
            json!({
                "before_ms": 1_780_000_000_000_u64,
                "limit": 100,
                "dry_run": false,
                "reason": "retention policy",
            })
        );
    }

    #[test]
    fn m18_promotion_paths_encode_segments_and_pagination() {
        assert_eq!(
            encoded_api_path(&["promotions", "evidence"], &[]),
            "/promotions/evidence"
        );
        assert_eq!(
            encoded_api_path(&["promotions", "evidence", "evidence-1"], &[]),
            "/promotions/evidence/evidence-1"
        );
        assert_eq!(
            encoded_api_path(&["promotions", "evidence", "evidence-1", "promote"], &[],),
            "/promotions/evidence/evidence-1/promote"
        );
        assert_eq!(
            encoded_api_path(&["promotions", "evidence", "evidence-1", "reject"], &[],),
            "/promotions/evidence/evidence-1/reject"
        );
        assert_eq!(
            encoded_api_path(&["promotions", "decisions"], &[]),
            "/promotions/decisions"
        );
        assert_eq!(
            encoded_api_path(&["promotions", "decisions", "decision-1"], &[]),
            "/promotions/decisions/decision-1"
        );
        assert_eq!(
            encoded_api_path(
                &["promotions", "current", "clinical-pharma", "canary-east"],
                &[],
            ),
            "/promotions/current/clinical-pharma/canary-east"
        );
        assert_eq!(
            encoded_api_path(
                &[
                    "promotions",
                    "current",
                    "clinical-pharma",
                    "canary-east",
                    "rollback",
                ],
                &[],
            ),
            "/promotions/current/clinical-pharma/canary-east/rollback"
        );
        assert_eq!(PROMOTION_STATUS_PATH, "/admin/promotions/status");
        assert_eq!(PROMOTION_RECONCILE_PATH, "/admin/promotions/reconcile");
        assert_eq!(PROMOTION_RECOVER_PATH, "/admin/promotions/recover");

        // Dynamic segments are encoded exactly once before Axum percent-decodes
        // them and the promotion manager applies its stricter segment policy.
        assert_eq!(
            encoded_api_path(
                &["promotions", "current", "clinical/pharma", "canary east"],
                &[],
            ),
            "/promotions/current/clinical%2Fpharma/canary%20east"
        );
        assert_eq!(
            encoded_api_path(&["promotions", "decisions", "decision/1?review"], &[],),
            "/promotions/decisions/decision%2F1%3Freview"
        );
        assert_eq!(
            promotion_page_path(
                &["promotions", "evidence"],
                Some(25),
                Some("v1.next_page-2"),
            ),
            "/promotions/evidence?limit=25&cursor=v1.next_page-2"
        );
    }

    #[test]
    fn m19_governance_paths_and_signed_intent_safety_match_server_contract() {
        assert_eq!(
            encoded_api_path(&["governance", "keys", "registration-1"], &[]),
            "/governance/keys/registration-1"
        );
        assert_eq!(
            encoded_api_path(&["governance", "keys", "registration/1", "revoke"], &[]),
            "/governance/keys/registration%2F1/revoke"
        );
        assert_eq!(
            promotion_page_path(
                &["governance", "revocations"],
                Some(10),
                Some("v1.revocation-2")
            ),
            "/governance/revocations?limit=10&cursor=v1.revocation-2"
        );
        assert_eq!(
            encoded_api_path(&["governance", "approvals", "approval/1"], &[]),
            "/governance/approvals/approval%2F1"
        );
        assert_eq!(
            encoded_api_path(&["governance", "bindings", "approval-1"], &[]),
            "/governance/bindings/approval-1"
        );
        assert_eq!(
            promotion_page_path(&["governance", "policies"], Some(25), Some("v1.policy-2")),
            "/governance/policies?limit=25&cursor=v1.policy-2"
        );
        assert_eq!(
            read_public_governance_json_argument(
                r#"{"statement":{"payload":{}},"promoter_signature":{"signature":"public-proof"}}"#,
                "signed promotion intent",
            )
            .unwrap(),
            json!({
                "statement": { "payload": {} },
                "promoter_signature": { "signature": "public-proof" },
            })
        );
        assert!(
            read_public_governance_json_argument(
                r#"{"statement":{},"signing_key":{"secret_key":"must-stay-local"}}"#,
                "signed policy revision",
            )
            .unwrap_err()
            .contains("only pre-signed public statements")
        );
        assert!(
            read_public_governance_json_argument(
                r#"{"nested":[{"Root-Private-Key":"must-stay-local"}]}"#,
                "signed governance key registration",
            )
            .is_err()
        );
        assert_eq!(
            promotion_reconcile_body("clinical/pharma", "canary east", true),
            json!({
                "target": {
                    "space_type": "clinical/pharma",
                    "channel": "canary east",
                },
                "dry_run": true,
            })
        );
    }

    #[test]
    fn m20_artifact_governance_paths_and_public_envelopes_match_server_contract() {
        assert_eq!(
            promotion_page_path(
                &["governance", "artifact-attestations"],
                Some(25),
                Some("v1.artifact-2"),
            ),
            "/governance/artifact-attestations?limit=25&cursor=v1.artifact-2"
        );
        assert_eq!(
            encoded_api_path(
                &["governance", "artifact-attestations", "attestation/1?audit"],
                &[],
            ),
            "/governance/artifact-attestations/attestation%2F1%3Faudit"
        );
        assert_eq!(
            read_public_governance_json_argument(
                r#"{"statement":{"payload":{"kind":"corpus"}},"attestor_signature":{"signature":"public-proof"}}"#,
                "signed artifact attestation",
            )
            .unwrap(),
            json!({
                "statement": { "payload": { "kind": "corpus" } },
                "attestor_signature": { "signature": "public-proof" },
            })
        );
        assert!(
            read_public_governance_json_argument(
                r#"{"attestation_ids":["a1"],"nested":{"private-key":"must-stay-local"}}"#,
                "artifact binding request",
            )
            .unwrap_err()
            .contains("only pre-signed public statements")
        );
    }

    #[test]
    fn m25_semantic_repair_paths_and_public_envelopes_match_server_contract() {
        assert_eq!(
            promotion_page_path(
                &["semantic-repairs", "revisions"],
                Some(8),
                Some("v1.revision-2"),
            ),
            "/semantic-repairs/revisions?limit=8&cursor=v1.revision-2"
        );
        assert_eq!(
            encoded_api_path(
                &[
                    "semantic-repairs",
                    "revisions",
                    "revision/1?audit",
                    "review",
                ],
                &[],
            ),
            "/semantic-repairs/revisions/revision%2F1%3Faudit/review"
        );
        assert_eq!(
            encoded_api_path(
                &[
                    "semantic-repairs",
                    "current",
                    "clinical/pharma",
                    "canary east",
                ],
                &[],
            ),
            "/semantic-repairs/current/clinical%2Fpharma/canary%20east"
        );
        assert_eq!(
            read_public_governance_json_argument(
                r#"{"statement":{"domain":"cognigraph.semantic-repair-revision.v1"},"author_signature":{"signature":"public-proof"}}"#,
                "signed semantic repair revision",
            )
            .unwrap(),
            json!({
                "statement": { "domain": "cognigraph.semantic-repair-revision.v1" },
                "author_signature": { "signature": "public-proof" },
            })
        );
        assert!(
            read_public_governance_json_argument(
                r#"{"statement":{},"approver_signature":{},"private_key":"must-stay-local"}"#,
                "signed semantic repair review",
            )
            .unwrap_err()
            .contains("only pre-signed public statements")
        );
    }

    #[test]
    fn m26_verified_semantic_repair_paths_and_requests_match_server_contract() {
        const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert_eq!(
            semantic_repair_generation_build_body("clinical/pharma", "canary east", HEAD),
            json!({
                "target": {
                    "space_type": "clinical/pharma",
                    "channel": "canary east",
                },
                "expected_promotion_head_decision_id": HEAD,
            })
        );
        assert_eq!(
            promotion_page_path(
                &["semantic-repairs", "generations"],
                Some(12),
                Some("v1.generation-2"),
            ),
            "/semantic-repairs/generations?limit=12&cursor=v1.generation-2"
        );
        assert_eq!(
            encoded_api_path(
                &[
                    "semantic-repairs",
                    "generations",
                    "generation/1?audit",
                    "deploy",
                ],
                &[],
            ),
            "/semantic-repairs/generations/generation%2F1%3Faudit/deploy"
        );
        assert_eq!(
            encoded_api_path(
                &[
                    "semantic-repairs",
                    "deployments",
                    "current",
                    "clinical/pharma",
                ],
                &[],
            ),
            "/semantic-repairs/deployments/current/clinical%2Fpharma"
        );
        assert_eq!(
            read_public_governance_json_argument(
                r#"{"statement":{"domain":"cognigraph.semantic-repair-deployment-intent.v1"},"promoter_signature":{"signature":"public-proof"}}"#,
                "signed semantic repair deployment intent",
            )
            .unwrap(),
            json!({
                "statement": { "domain": "cognigraph.semantic-repair-deployment-intent.v1" },
                "promoter_signature": { "signature": "public-proof" },
            })
        );
        assert!(
            read_public_governance_json_argument(
                r#"{"statement":{},"promoter_signature":{},"private-key":"must-stay-local"}"#,
                "signed semantic repair deployment intent",
            )
            .unwrap_err()
            .contains("only pre-signed public statements")
        );
    }

    #[test]
    fn custody_output_files_publish_complete_bytes_without_replacement() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("receipt.json");
        let output_text = output.to_string_lossy();

        write_new_private_file(&output_text, b"first").unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"first");
        assert!(write_new_private_file(&output_text, b"second").is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"first");
        assert!(std::fs::read_dir(root.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".partial")
        }));

        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, symlink};

            assert_eq!(std::fs::metadata(&output).unwrap().mode() & 0o077, 0);
            let linked_parent = root.path().join("linked-parent");
            symlink(root.path(), &linked_parent).unwrap();
            assert!(
                write_new_private_file(
                    &linked_parent.join("unsafe.json").to_string_lossy(),
                    b"unsafe",
                )
                .is_err()
            );
        }
    }

    #[test]
    fn restore_receipt_must_remain_outside_the_cas_and_bundle() {
        let root = tempfile::tempdir().unwrap();
        let cas = root.path().join("cas");
        let bundle = root.path().join("bundle");
        let external = root.path().join("receipts");
        std::fs::create_dir_all(cas.join("tenants")).unwrap();
        std::fs::create_dir(&bundle).unwrap();
        std::fs::create_dir(&external).unwrap();

        let inside_cas = cas.join("restore.json").to_string_lossy().into_owned();
        let inside_bundle = bundle.join("restore.json").to_string_lossy().into_owned();
        let outside = external.join("restore.json").to_string_lossy().into_owned();
        assert!(
            require_output_outside_directory(&inside_cas, &cas.to_string_lossy(), "CAS").is_err()
        );
        assert!(
            require_output_outside_directory(&inside_bundle, &bundle.to_string_lossy(), "bundle")
                .is_err()
        );
        assert!(require_output_outside_directory(&outside, &cas.to_string_lossy(), "CAS").is_ok());
        assert!(
            require_output_outside_directory(&outside, &bundle.to_string_lossy(), "bundle").is_ok()
        );
    }
}
