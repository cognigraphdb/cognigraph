//! Hand-rolled argument parsing — the CLI's whole surface is small enough
//! that a parser dependency would outweigh it. Pure function, unit-tested.

#[derive(Debug, Clone, PartialEq)]
pub struct Invocation {
    /// Server base URL (--url / COGNIGRAPH_URL / default localhost:3000).
    pub url: Option<String>,
    /// Bearer token (--token / COGNIGRAPH_TOKEN).
    pub token: Option<String>,
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help,
    Health,
    ReferencesAudit {
        snapshot: String,
    },
    ReferencesRepair {
        snapshot: String,
        plan: String,
        out: String,
    },
    Export {
        out: Option<String>,
    },
    Import {
        file: Option<String>,
    },
    Query {
        query: String,
        write: bool,
        binds: Vec<(String, serde_json::Value)>,
    },
    DocGet {
        collection: String,
        key: String,
    },
    DocList {
        collection: String,
        limit: Option<u64>,
        offset: Option<u64>,
    },
    DocPut {
        collection: String,
        document: String,
    },
    DocDelete {
        collection: String,
        key: String,
    },
    UserCreate {
        username: String,
        role: String,
        tenant: Option<String>,
    },
    UserList,
    UserDelete {
        key: String,
    },
    TokenIssue {
        user: String,
        ttl_secs: Option<u64>,
    },
    TokenList {
        user: String,
    },
    TokenRevoke {
        user: String,
        token: String,
    },
    TokenRotate {
        user: String,
        token: String,
        ttl_secs: Option<u64>,
    },
    Embed {
        collection: String,
        file: String,
        text_field: Option<String>,
        batch_size: Option<u64>,
    },
    CacheStats,
    CacheClear,
    NeuronList {
        space: Option<String>,
        status: Option<String>,
    },
    NeuronPropose {
        document: String,
    },
    NeuronTransition {
        key: String,
        action: String,
        note: Option<String>,
    },
    NeuronPending {
        space: Option<String>,
        limit: Option<u64>,
    },
    NeuronReview {
        space: String,
    },
    NeuronAudit {
        space: String,
    },
    NeuronShow {
        key: String,
    },
    Advise {
        space: String,
    },
    Draft {
        space: String,
        file: String,
    },
    TenantList,
    TenantCreate {
        name: String,
    },
    TenantBootstrapAdmin {
        name: String,
        username: String,
    },
    TenantStatus {
        name: String,
        status: String,
    },
    TenantDelete {
        name: String,
    },
    JobSubmit {
        kind: String,
        input: String,
        idempotency_key: String,
    },
    JobList {
        kind: Option<String>,
        status: Option<String>,
        limit: Option<u64>,
        cursor: Option<String>,
        archived: Option<String>,
        offset: Option<u64>,
    },
    JobQueueStatus,
    JobReconcile {
        dry_run: bool,
        limit: Option<u64>,
        cursor: Option<String>,
    },
    JobArchive {
        before_ms: u64,
        limit: Option<u64>,
        dry_run: bool,
        reason: Option<String>,
    },
    JobStatus {
        id: String,
    },
    JobCancel {
        id: String,
        reason: Option<String>,
    },
    JobRetry {
        id: String,
        idempotency_key: String,
        mode: Option<String>,
        reason: Option<String>,
    },
    PromotionEvidenceCreate {
        evidence: String,
        idempotency_key: String,
    },
    PromotionEvidenceList {
        limit: Option<u64>,
        cursor: Option<String>,
    },
    PromotionEvidenceShow {
        id: String,
    },
    PromotionPromote {
        evidence_id: String,
        intent: String,
        idempotency_key: String,
    },
    PromotionReject {
        evidence_id: String,
        intent: String,
        idempotency_key: String,
    },
    PromotionDecisions {
        limit: Option<u64>,
        cursor: Option<String>,
    },
    PromotionDecisionShow {
        id: String,
    },
    PromotionCurrent {
        space_type: String,
        channel: String,
    },
    PromotionRollback {
        space_type: String,
        channel: String,
        intent: String,
        idempotency_key: String,
    },
    GovernanceStatus,
    GovernanceKeyList {
        limit: Option<u64>,
        cursor: Option<String>,
    },
    GovernanceKeyRegister {
        request: String,
        idempotency_key: String,
    },
    GovernanceKeyShow {
        id: String,
    },
    GovernanceKeyRevoke {
        registration_id: String,
        request: String,
        idempotency_key: String,
    },
    GovernanceRevocationList {
        limit: Option<u64>,
        cursor: Option<String>,
    },
    GovernanceRevocationShow {
        id: String,
    },
    GovernancePolicyList {
        limit: Option<u64>,
        cursor: Option<String>,
    },
    GovernancePolicyCreate {
        request: String,
        idempotency_key: String,
    },
    GovernancePolicyShow {
        id: String,
    },
    GovernancePolicyApprove {
        id: String,
        request: String,
        idempotency_key: String,
    },
    GovernanceApprovalShow {
        id: String,
    },
    GovernanceBindingResolve {
        approval_id: String,
    },
    GovernanceArtifactAttest {
        request: String,
        idempotency_key: String,
    },
    GovernanceArtifactList {
        limit: Option<u64>,
        cursor: Option<String>,
    },
    GovernanceArtifactShow {
        id: String,
    },
    GovernanceArtifactBindingResolve {
        request: String,
    },
    SemanticRepairRevisionSubmit {
        request: String,
        idempotency_key: String,
    },
    SemanticRepairRevisionList {
        limit: Option<u64>,
        cursor: Option<String>,
    },
    SemanticRepairRevisionShow {
        id: String,
    },
    SemanticRepairReviewSubmit {
        revision_id: String,
        request: String,
        idempotency_key: String,
    },
    SemanticRepairReviewList {
        limit: Option<u64>,
        cursor: Option<String>,
    },
    SemanticRepairReviewShow {
        id: String,
    },
    SemanticRepairCurrent {
        space_type: String,
        channel: String,
    },
    SemanticRepairGenerationBuild {
        target_space: String,
        channel: String,
        expected_promotion_head_decision_id: String,
        idempotency_key: String,
    },
    SemanticRepairGenerationList {
        limit: Option<u64>,
        cursor: Option<String>,
    },
    SemanticRepairGenerationShow {
        id: String,
    },
    SemanticRepairGenerationDeploy {
        id: String,
        request: String,
        idempotency_key: String,
    },
    SemanticRepairDeploymentCurrent {
        space_type: String,
    },
    ArtifactCustodyPlan {
        evidence_id: String,
        out: Option<String>,
    },
    ArtifactCustodyCreate {
        evidence_id: String,
        cas_root: String,
        out: String,
        max_bytes: Option<u64>,
    },
    ArtifactCustodyVerify {
        bundle: String,
        expected_plan_digest: String,
        tenant: String,
        incarnation: String,
        max_bytes: Option<u64>,
    },
    ArtifactCustodyRestore {
        bundle: String,
        cas_root: String,
        expected_plan_digest: String,
        tenant: String,
        incarnation: String,
        receipt: String,
        max_bytes: Option<u64>,
    },
    PromotionStatus,
    PromotionRecover,
    PromotionReconcile {
        space_type: String,
        channel: String,
        dry_run: bool,
    },
    DraftAccept {
        space: String,
    },
    NeuronGraduation {
        space: String,
    },
    Login {
        username: String,
    },
}

#[cfg(not(feature = "enterprise"))]
pub const USAGE: &str = include_str!("usage-community.txt");

#[cfg(feature = "enterprise")]
pub const USAGE: &str = "\
cognigraph — CogniGraph server administration

USAGE: cognigraph [--url URL] [--token TOKEN] <command>

Connection (flags override env):
  --url URL       server base URL   [env COGNIGRAPH_URL, default http://127.0.0.1:3000]
  --token TOKEN   bearer token      [env COGNIGRAPH_TOKEN]

COMMANDS
  health                          liveness + backend readiness
  export [--out FILE]             hot JSON snapshot (stdout by default)
  import [FILE]                   restore a snapshot (stdin if no FILE)
  references audit SNAPSHOT       bounded offline identity/reference report
  references repair SNAPSHOT PLAN --out FILE
                                  offline explicit repair to a NEW snapshot
  query [--write] [--bind k=JSON]... QUERY
                                  run CGQL (EXPLAIN supported; --write for mutations)
  doc get COLLECTION KEY
  doc list COLLECTION [--limit N] [--offset N]
  doc put COLLECTION JSON|@FILE   create a document
  doc delete COLLECTION KEY
  user create USERNAME ROLE [--tenant T]   password from COGNIGRAPH_PASSWORD or stdin
  user list
  user delete KEY
  token issue USERKEY [--ttl-secs N]   secret printed once (0 = non-expiring)
  token list USERKEY
  token revoke USERKEY TOKENKEY
  token rotate USERKEY TOKENKEY [--ttl-secs N]   same key, fresh secret
  embed COLLECTION FILE.jsonl [--text-field F] [--batch-size N]
                                  server-side embed + atomic store (H8)
  cache stats | cache clear
  neuron list [--space S] [--status proposed|accepted|rejected|retired]
  neuron propose JSON|@FILE       validated authoring (stored as proposed)
  neuron pending [SPACE] [--limit N]   the review queue, digested (oldest first)
  neuron review SPACE             judge proposals + apply the review policy
  neuron audit SPACE              sampled auto-accepts awaiting human audit
  neuron show KEY                 a neuron WITH the evidence its triggers match
  neuron accept|reject|retire KEY [--note \"...\"]   lifecycle transitions
  neuron graduation SPACE         leave-one-out redundancy candidates
  advise SPACE                    gate advisor: where require_in_sentence is safe/needed
  draft SPACE CHUNKS.jsonl        draft a NEW space type from chunks (inert until accepted)
  draft accept SPACE              accept a reviewed draft into space_types (attributed)
  tenant list|create NAME|suspend NAME|activate NAME|delete NAME
                                  tenant lifecycle (host-admin / TenantAdmin scope)
  tenant bootstrap-admin NAME USERNAME
                                  provision first tenant Admin; password from COGNIGRAPH_PASSWORD or stdin
  job submit KIND JSON|@FILE --idempotency-key KEY
                                  submit durable governed work idempotently
  job list [--kind K] [--status S] [--limit N] [--cursor C]
           [--archived exclude|include|only] [--offset N]
                                  cursor pagination is preferred; --offset is deprecated
  job queue-status                tenant queue, quota, catalog, and archive status (Admin)
  job reconcile [--dry-run] [--limit N] [--cursor C]
                                  inspect or repair tenant job catalog drift (Admin)
  job archive --before-ms N [--limit N] [--dry-run] [--reason TEXT]
                                  archive terminal jobs older than the cutoff (Admin)
  job status ID
  job cancel ID [--reason TEXT]
  job retry ID --idempotency-key KEY [--mode resume|restart] [--reason TEXT]
  promotion evidence create JSON|@FILE --idempotency-key KEY
  promotion evidence list [--limit N] [--cursor C]
  promotion evidence show ID
  promotion promote EVIDENCE SIGNED_INTENT_JSON|@FILE --idempotency-key KEY
  promotion reject EVIDENCE SIGNED_INTENT_JSON|@FILE --idempotency-key KEY
  promotion decisions [--limit N] [--cursor C]
  promotion decision show ID
  promotion current SPACE_TYPE CHANNEL
  promotion rollback SPACE_TYPE CHANNEL SIGNED_INTENT_JSON|@FILE --idempotency-key KEY
  promotion status
  promotion recover
  promotion reconcile SPACE_TYPE CHANNEL [--dry-run]
  governance status
  governance key list [--limit N] [--cursor C]
  governance key register JSON|@FILE --idempotency-key KEY
  governance key show ID
  governance key revoke REGISTRATION_ID JSON|@FILE --idempotency-key KEY
  governance revocation list [--limit N] [--cursor C]
  governance revocation show ID
  governance policy list [--limit N] [--cursor C]
  governance policy create JSON|@FILE --idempotency-key KEY
  governance policy show ID
  governance policy approve ID JSON|@FILE --idempotency-key KEY
  governance approval show ID
  governance binding resolve APPROVAL_ID
  governance artifact attest JSON|@FILE --idempotency-key KEY
  governance artifact list [--limit N] [--cursor C]
  governance artifact show ID
  governance artifact binding resolve JSON|@FILE
                                  only public keys and pre-signed statements are sent
  semantic-repair revision submit JSON|@FILE --idempotency-key KEY
  semantic-repair revision list [--limit N] [--cursor C]
  semantic-repair revision show ID
  semantic-repair review submit REVISION_ID JSON|@FILE --idempotency-key KEY
  semantic-repair review list [--limit N] [--cursor C]
  semantic-repair review show ID
  semantic-repair current SPACE_TYPE CHANNEL
                                  submit only externally signed public envelopes; resolve requires the promoted exact digest
  semantic-repair generation build --target-space SPACE_TYPE --channel CHANNEL
                                    --expected-promotion-head DECISION_ID --idempotency-key KEY
  semantic-repair generation list [--limit N] [--cursor C]
  semantic-repair generation show ID
  semantic-repair generation deploy ID SIGNED_INTENT_JSON|@FILE --idempotency-key KEY
  semantic-repair deployment current SPACE_TYPE
                                  build and deploy only against explicitly pinned governed authority
  artifact custody plan EVIDENCE [--out FILE]
                                  fetch the deterministic Admin recovery plan
  artifact custody create EVIDENCE --cas-root DIR --out BUNDLE [--max-bytes N]
                                  copy and rehash a closed portable CAS bundle
  artifact custody verify BUNDLE --expected-plan-digest SHA256 --tenant T --incarnation I [--max-bytes N]
                                  verify the closed bundle without restoring it
  artifact custody restore BUNDLE --cas-root DIR --expected-plan-digest SHA256
                           --tenant T --incarnation I --receipt FILE [--max-bytes N]
                                  atomically restore an absent tenant-incarnation CAS scope
  login USERNAME                  print a JWT (password like `user create`)
";

pub fn parse(args: &[String]) -> Result<Invocation, String> {
    let mut url = None;
    let mut token = None;
    let mut rest: Vec<&str> = Vec::new();
    let mut iter = args.iter().map(String::as_str);
    while let Some(arg) = iter.next() {
        match arg {
            "--url" => url = Some(required_value(&mut iter, "--url")?),
            "--token" => token = Some(required_value(&mut iter, "--token")?),
            "--help" | "-h" | "help" if rest.is_empty() => {
                return Ok(Invocation {
                    url: None,
                    token: None,
                    command: Command::Help,
                });
            }
            other => rest.push(other),
        }
    }
    let command = parse_command(&rest)?;
    Ok(Invocation {
        url,
        token,
        command,
    })
}

fn required_value<'a>(
    iter: &mut impl Iterator<Item = &'a str>,
    flag: &str,
) -> Result<String, String> {
    iter.next()
        .map(str::to_string)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_cursor(value: &str) -> Result<String, String> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b':')
        })
    {
        return Err("--cursor must be non-empty URL-safe ASCII ([A-Za-z0-9._~:-])".to_string());
    }
    Ok(value.to_string())
}

fn nonempty(value: &str, label: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(value.to_string())
    }
}

fn parse_pagination_flags(tail: &[&str]) -> Result<(Option<u64>, Option<String>), String> {
    let mut limit = None;
    let mut cursor = None;
    let mut iter = tail.iter();
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match *flag {
            "--limit" => {
                let value = value
                    .parse()
                    .map_err(|_| format!("--limit expects a number, got `{value}`"))?;
                if limit.replace(value).is_some() {
                    return Err("--limit may only be specified once".into());
                }
            }
            "--cursor" => {
                let value = parse_cursor(value)?;
                if cursor.replace(value).is_some() {
                    return Err("--cursor may only be specified once".into());
                }
            }
            other => return Err(format!("unknown flag `{other}`")),
        }
    }
    Ok((limit, cursor))
}

fn parse_idempotency_flag(tail: &[&str], command: &str) -> Result<String, String> {
    let mut idempotency_key = None;
    let mut iter = tail.iter();
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match *flag {
            "--idempotency-key" => {
                let value = nonempty(value, "--idempotency-key")?;
                if idempotency_key.replace(value).is_some() {
                    return Err("--idempotency-key may only be specified once".into());
                }
            }
            other => return Err(format!("unknown flag `{other}`")),
        }
    }
    idempotency_key.ok_or_else(|| format!("{command} requires --idempotency-key KEY"))
}

fn parse_command(rest: &[&str]) -> Result<Command, String> {
    let unexpected = |args: &[&str]| format!("unexpected arguments: {}", args.join(" "));
    match rest {
        [] => Ok(Command::Help),
        ["health"] => Ok(Command::Health),
        ["references", "audit", snapshot] => Ok(Command::ReferencesAudit {
            snapshot: snapshot.to_string(),
        }),
        ["references", "repair", snapshot, plan, "--out", out] => Ok(Command::ReferencesRepair {
            snapshot: snapshot.to_string(),
            plan: plan.to_string(),
            out: out.to_string(),
        }),
        ["export", tail @ ..] => match tail {
            [] => Ok(Command::Export { out: None }),
            ["--out", file] => Ok(Command::Export {
                out: Some(file.to_string()),
            }),
            other => Err(unexpected(other)),
        },
        ["import"] => Ok(Command::Import { file: None }),
        ["import", file] => Ok(Command::Import {
            file: Some(file.to_string()),
        }),
        ["query", tail @ ..] => parse_query(tail),
        ["doc", "get", collection, key] => Ok(Command::DocGet {
            collection: collection.to_string(),
            key: key.to_string(),
        }),
        ["doc", "list", collection, tail @ ..] => {
            let mut limit = None;
            let mut offset = None;
            let mut iter = tail.iter();
            while let Some(flag) = iter.next() {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                let parsed: u64 = value
                    .parse()
                    .map_err(|_| format!("{flag} expects a number, got `{value}`"))?;
                match *flag {
                    "--limit" => limit = Some(parsed),
                    "--offset" => offset = Some(parsed),
                    other => return Err(format!("unknown flag `{other}`")),
                }
            }
            Ok(Command::DocList {
                collection: collection.to_string(),
                limit,
                offset,
            })
        }
        ["doc", "put", collection, document] => Ok(Command::DocPut {
            collection: collection.to_string(),
            document: document.to_string(),
        }),
        ["doc", "delete", collection, key] => Ok(Command::DocDelete {
            collection: collection.to_string(),
            key: key.to_string(),
        }),
        ["user", "create", username, role, tail @ ..] => {
            let tenant = match tail {
                [] => None,
                ["--tenant", value] => Some(value.to_string()),
                other => return Err(unexpected(other)),
            };
            Ok(Command::UserCreate {
                username: username.to_string(),
                role: role.to_string(),
                tenant,
            })
        }
        ["user", "list"] => Ok(Command::UserList),
        ["user", "delete", key] => Ok(Command::UserDelete {
            key: key.to_string(),
        }),
        ["token", "issue", user, tail @ ..] => Ok(Command::TokenIssue {
            user: user.to_string(),
            ttl_secs: parse_ttl_flag(tail)?,
        }),
        ["token", "list", user] => Ok(Command::TokenList {
            user: user.to_string(),
        }),
        ["token", "revoke", user, token] => Ok(Command::TokenRevoke {
            user: user.to_string(),
            token: token.to_string(),
        }),
        ["token", "rotate", user, token, tail @ ..] => Ok(Command::TokenRotate {
            user: user.to_string(),
            token: token.to_string(),
            ttl_secs: parse_ttl_flag(tail)?,
        }),
        ["embed", collection, file, tail @ ..] => {
            let mut text_field = None;
            let mut batch_size = None;
            let mut iter = tail.iter();
            while let Some(flag) = iter.next() {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match *flag {
                    "--text-field" => text_field = Some(value.to_string()),
                    "--batch-size" => {
                        batch_size = Some(value.parse().map_err(|_| {
                            format!("--batch-size expects a number, got `{value}`")
                        })?);
                    }
                    other => return Err(format!("unknown flag `{other}`")),
                }
            }
            Ok(Command::Embed {
                collection: collection.to_string(),
                file: file.to_string(),
                text_field,
                batch_size,
            })
        }
        ["neuron", "list", tail @ ..] => {
            let mut space = None;
            let mut status = None;
            let mut iter = tail.iter();
            while let Some(flag) = iter.next() {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match *flag {
                    "--space" => space = Some(value.to_string()),
                    "--status" => status = Some(value.to_string()),
                    other => return Err(format!("unknown flag `{other}`")),
                }
            }
            Ok(Command::NeuronList { space, status })
        }
        ["neuron", "propose", document] => Ok(Command::NeuronPropose {
            document: document.to_string(),
        }),
        ["neuron", "review", space] => Ok(Command::NeuronReview {
            space: space.to_string(),
        }),
        ["neuron", "audit", space] => Ok(Command::NeuronAudit {
            space: space.to_string(),
        }),
        ["neuron", "pending", tail @ ..] => {
            let mut space = None;
            let mut limit = None;
            let mut iter = tail.iter();
            while let Some(arg) = iter.next() {
                match *arg {
                    "--limit" => {
                        limit = Some(
                            iter.next()
                                .and_then(|v| v.parse().ok())
                                .ok_or("--limit expects a number")?,
                        );
                    }
                    other if space.is_none() => space = Some(other.to_string()),
                    other => return Err(format!("unexpected argument `{other}`")),
                }
            }
            Ok(Command::NeuronPending { space, limit })
        }
        ["neuron", "show", key] => Ok(Command::NeuronShow {
            key: key.to_string(),
        }),
        [
            "neuron",
            action @ ("accept" | "reject" | "retire"),
            key,
            tail @ ..,
        ] => {
            let note = match tail {
                [] => None,
                ["--note", value] => Some(value.to_string()),
                other => return Err(format!("unexpected arguments: {}", other.join(" "))),
            };
            Ok(Command::NeuronTransition {
                key: key.to_string(),
                action: action.to_string(),
                note,
            })
        }
        ["neuron", "graduation", space] => Ok(Command::NeuronGraduation {
            space: space.to_string(),
        }),
        ["advise", space] => Ok(Command::Advise {
            space: space.to_string(),
        }),
        ["draft", "accept", space] => Ok(Command::DraftAccept {
            space: space.to_string(),
        }),
        ["tenant", "list"] => Ok(Command::TenantList),
        ["tenant", "create", name] => Ok(Command::TenantCreate {
            name: name.to_string(),
        }),
        ["tenant", "bootstrap-admin", name, username] => Ok(Command::TenantBootstrapAdmin {
            name: name.to_string(),
            username: username.to_string(),
        }),
        ["tenant", action @ ("suspend" | "activate"), name] => Ok(Command::TenantStatus {
            name: name.to_string(),
            status: if *action == "suspend" {
                "suspended".into()
            } else {
                "active".into()
            },
        }),
        ["tenant", "delete", name] => Ok(Command::TenantDelete {
            name: name.to_string(),
        }),
        ["job", "submit", kind, input, tail @ ..] => {
            let mut idempotency_key = None;
            let mut iter = tail.iter();
            while let Some(flag) = iter.next() {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match *flag {
                    "--idempotency-key" => {
                        if idempotency_key.replace(value.to_string()).is_some() {
                            return Err("--idempotency-key may only be specified once".into());
                        }
                    }
                    other => return Err(format!("unknown flag `{other}`")),
                }
            }
            let idempotency_key = idempotency_key
                .filter(|key| !key.is_empty())
                .ok_or("job submit requires --idempotency-key KEY")?;
            Ok(Command::JobSubmit {
                kind: kind.to_string(),
                input: input.to_string(),
                idempotency_key,
            })
        }
        ["job", "list", tail @ ..] => {
            let mut kind = None;
            let mut status = None;
            let mut limit = None;
            let mut cursor = None;
            let mut archived = None;
            let mut offset = None;
            let mut iter = tail.iter();
            while let Some(flag) = iter.next() {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match *flag {
                    "--kind" => kind = Some(value.to_string()),
                    "--status" => status = Some(value.to_string()),
                    "--limit" => {
                        limit = Some(
                            value
                                .parse()
                                .map_err(|_| format!("--limit expects a number, got `{value}`"))?,
                        );
                    }
                    "--cursor" => cursor = Some(parse_cursor(value)?),
                    "--archived" if matches!(*value, "exclude" | "include" | "only") => {
                        archived = Some(value.to_string());
                    }
                    "--archived" => {
                        return Err(format!(
                            "--archived expects `exclude`, `include`, or `only`, got `{value}`"
                        ));
                    }
                    "--offset" => {
                        offset =
                            Some(value.parse().map_err(|_| {
                                format!("--offset expects a number, got `{value}`")
                            })?);
                    }
                    other => return Err(format!("unknown flag `{other}`")),
                }
            }
            if cursor.is_some() && offset.is_some() {
                return Err("--cursor and deprecated --offset are mutually exclusive".into());
            }
            Ok(Command::JobList {
                kind,
                status,
                limit,
                cursor,
                archived,
                offset,
            })
        }
        ["job", "queue-status"] => Ok(Command::JobQueueStatus),
        ["job", "reconcile", tail @ ..] => {
            let mut dry_run = false;
            let mut limit = None;
            let mut cursor = None;
            let mut iter = tail.iter();
            while let Some(flag) = iter.next() {
                match *flag {
                    "--dry-run" => dry_run = true,
                    "--limit" => {
                        let value = iter
                            .next()
                            .ok_or_else(|| "--limit requires a value".to_string())?;
                        limit = Some(
                            value
                                .parse()
                                .map_err(|_| format!("--limit expects a number, got `{value}`"))?,
                        );
                    }
                    "--cursor" => {
                        let value = iter
                            .next()
                            .ok_or_else(|| "--cursor requires a value".to_string())?;
                        cursor = Some(parse_cursor(value)?);
                    }
                    other => return Err(format!("unknown flag `{other}`")),
                }
            }
            Ok(Command::JobReconcile {
                dry_run,
                limit,
                cursor,
            })
        }
        ["job", "archive", tail @ ..] => {
            let mut before_ms = None;
            let mut limit = None;
            let mut dry_run = false;
            let mut reason = None;
            let mut iter = tail.iter();
            while let Some(flag) = iter.next() {
                match *flag {
                    "--dry-run" => dry_run = true,
                    "--before-ms" => {
                        let value = iter
                            .next()
                            .ok_or_else(|| "--before-ms requires a value".to_string())?;
                        let parsed = value
                            .parse()
                            .map_err(|_| format!("--before-ms expects a number, got `{value}`"))?;
                        if before_ms.replace(parsed).is_some() {
                            return Err("--before-ms may only be specified once".into());
                        }
                    }
                    "--limit" => {
                        let value = iter
                            .next()
                            .ok_or_else(|| "--limit requires a value".to_string())?;
                        limit = Some(
                            value
                                .parse()
                                .map_err(|_| format!("--limit expects a number, got `{value}`"))?,
                        );
                    }
                    "--reason" => {
                        let value = iter
                            .next()
                            .ok_or_else(|| "--reason requires a value".to_string())?;
                        reason = Some(value.to_string());
                    }
                    other => return Err(format!("unknown flag `{other}`")),
                }
            }
            Ok(Command::JobArchive {
                before_ms: before_ms.ok_or("job archive requires --before-ms N")?,
                limit,
                dry_run,
                reason,
            })
        }
        ["job", "status", id] => Ok(Command::JobStatus { id: id.to_string() }),
        ["job", "cancel", id, tail @ ..] => {
            let reason = match tail {
                [] => None,
                ["--reason", reason] => Some(reason.to_string()),
                other => return Err(unexpected(other)),
            };
            Ok(Command::JobCancel {
                id: id.to_string(),
                reason,
            })
        }
        ["job", "retry", id, tail @ ..] => {
            let mut idempotency_key = None;
            let mut mode = None;
            let mut reason = None;
            let mut iter = tail.iter();
            while let Some(flag) = iter.next() {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match *flag {
                    "--idempotency-key" => {
                        if idempotency_key.replace(value.to_string()).is_some() {
                            return Err("--idempotency-key may only be specified once".into());
                        }
                    }
                    "--mode" if matches!(*value, "resume" | "restart") => {
                        mode = Some(value.to_string());
                    }
                    "--mode" => {
                        return Err(format!(
                            "--mode expects `resume` or `restart`, got `{value}`"
                        ));
                    }
                    "--reason" => reason = Some(value.to_string()),
                    other => return Err(format!("unknown flag `{other}`")),
                }
            }
            let idempotency_key = idempotency_key
                .filter(|key| !key.is_empty())
                .ok_or("job retry requires --idempotency-key KEY")?;
            Ok(Command::JobRetry {
                id: id.to_string(),
                idempotency_key,
                mode,
                reason,
            })
        }
        ["promotion", "evidence", "create", evidence, tail @ ..] => {
            Ok(Command::PromotionEvidenceCreate {
                evidence: nonempty(evidence, "promotion evidence")?,
                idempotency_key: parse_idempotency_flag(tail, "promotion evidence create")?,
            })
        }
        ["promotion", "evidence", "list", tail @ ..] => {
            let (limit, cursor) = parse_pagination_flags(tail)?;
            Ok(Command::PromotionEvidenceList { limit, cursor })
        }
        ["promotion", "evidence", "show", id] => Ok(Command::PromotionEvidenceShow {
            id: nonempty(id, "evidence ID")?,
        }),
        ["promotion", "promote", evidence_id, intent, tail @ ..] => Ok(Command::PromotionPromote {
            evidence_id: nonempty(evidence_id, "evidence ID")?,
            intent: nonempty(intent, "signed promotion intent")?,
            idempotency_key: parse_idempotency_flag(tail, "promotion promote")?,
        }),
        ["promotion", "reject", evidence_id, intent, tail @ ..] => Ok(Command::PromotionReject {
            evidence_id: nonempty(evidence_id, "evidence ID")?,
            intent: nonempty(intent, "signed promotion intent")?,
            idempotency_key: parse_idempotency_flag(tail, "promotion reject")?,
        }),
        ["promotion", "decisions", tail @ ..] => {
            let (limit, cursor) = parse_pagination_flags(tail)?;
            Ok(Command::PromotionDecisions { limit, cursor })
        }
        ["promotion", "decision", "show", id] => Ok(Command::PromotionDecisionShow {
            id: nonempty(id, "decision ID")?,
        }),
        ["promotion", "current", space_type, channel] => Ok(Command::PromotionCurrent {
            space_type: nonempty(space_type, "space type")?,
            channel: nonempty(channel, "channel")?,
        }),
        [
            "promotion",
            "rollback",
            space_type,
            channel,
            intent,
            tail @ ..,
        ] => Ok(Command::PromotionRollback {
            space_type: nonempty(space_type, "space type")?,
            channel: nonempty(channel, "channel")?,
            intent: nonempty(intent, "signed promotion intent")?,
            idempotency_key: parse_idempotency_flag(tail, "promotion rollback")?,
        }),
        ["governance", "status"] => Ok(Command::GovernanceStatus),
        ["governance", "key", "list", tail @ ..] => {
            let (limit, cursor) = parse_pagination_flags(tail)?;
            Ok(Command::GovernanceKeyList { limit, cursor })
        }
        ["governance", "key", "register", request, tail @ ..] => {
            Ok(Command::GovernanceKeyRegister {
                request: nonempty(request, "governance key registration")?,
                idempotency_key: parse_idempotency_flag(tail, "governance key register")?,
            })
        }
        ["governance", "key", "show", id] => Ok(Command::GovernanceKeyShow {
            id: nonempty(id, "governance key registration ID")?,
        }),
        [
            "governance",
            "key",
            "revoke",
            registration_id,
            request,
            tail @ ..,
        ] => Ok(Command::GovernanceKeyRevoke {
            registration_id: nonempty(registration_id, "governance key registration ID")?,
            request: nonempty(request, "governance key revocation")?,
            idempotency_key: parse_idempotency_flag(tail, "governance key revoke")?,
        }),
        ["governance", "revocation", "list", tail @ ..] => {
            let (limit, cursor) = parse_pagination_flags(tail)?;
            Ok(Command::GovernanceRevocationList { limit, cursor })
        }
        ["governance", "revocation", "show", id] => Ok(Command::GovernanceRevocationShow {
            id: nonempty(id, "governance key revocation ID")?,
        }),
        ["governance", "policy", "list", tail @ ..] => {
            let (limit, cursor) = parse_pagination_flags(tail)?;
            Ok(Command::GovernancePolicyList { limit, cursor })
        }
        ["governance", "policy", "create", request, tail @ ..] => {
            Ok(Command::GovernancePolicyCreate {
                request: nonempty(request, "signed policy revision")?,
                idempotency_key: parse_idempotency_flag(tail, "governance policy create")?,
            })
        }
        ["governance", "policy", "show", id] => Ok(Command::GovernancePolicyShow {
            id: nonempty(id, "policy revision ID")?,
        }),
        ["governance", "policy", "approve", id, request, tail @ ..] => {
            Ok(Command::GovernancePolicyApprove {
                id: nonempty(id, "policy revision ID")?,
                request: nonempty(request, "signed policy approval")?,
                idempotency_key: parse_idempotency_flag(tail, "governance policy approve")?,
            })
        }
        ["governance", "approval", "show", id] => Ok(Command::GovernanceApprovalShow {
            id: nonempty(id, "policy approval ID")?,
        }),
        ["governance", "binding", "resolve", approval_id] => {
            Ok(Command::GovernanceBindingResolve {
                approval_id: nonempty(approval_id, "policy approval ID")?,
            })
        }
        ["governance", "artifact", "attest", request, tail @ ..] => {
            Ok(Command::GovernanceArtifactAttest {
                request: nonempty(request, "signed artifact attestation")?,
                idempotency_key: parse_idempotency_flag(tail, "governance artifact attest")?,
            })
        }
        ["governance", "artifact", "list", tail @ ..] => {
            let (limit, cursor) = parse_pagination_flags(tail)?;
            Ok(Command::GovernanceArtifactList { limit, cursor })
        }
        ["governance", "artifact", "show", id] => Ok(Command::GovernanceArtifactShow {
            id: nonempty(id, "artifact attestation ID")?,
        }),
        ["governance", "artifact", "binding", "resolve", request] => {
            Ok(Command::GovernanceArtifactBindingResolve {
                request: nonempty(request, "artifact binding request")?,
            })
        }
        ["semantic-repair", "revision", "submit", request, tail @ ..] => {
            Ok(Command::SemanticRepairRevisionSubmit {
                request: nonempty(request, "signed semantic repair revision")?,
                idempotency_key: parse_idempotency_flag(tail, "semantic-repair revision submit")?,
            })
        }
        ["semantic-repair", "revision", "list", tail @ ..] => {
            let (limit, cursor) = parse_pagination_flags(tail)?;
            Ok(Command::SemanticRepairRevisionList { limit, cursor })
        }
        ["semantic-repair", "revision", "show", id] => Ok(Command::SemanticRepairRevisionShow {
            id: nonempty(id, "semantic repair revision ID")?,
        }),
        [
            "semantic-repair",
            "review",
            "submit",
            revision_id,
            request,
            tail @ ..,
        ] => Ok(Command::SemanticRepairReviewSubmit {
            revision_id: nonempty(revision_id, "semantic repair revision ID")?,
            request: nonempty(request, "signed semantic repair review")?,
            idempotency_key: parse_idempotency_flag(tail, "semantic-repair review submit")?,
        }),
        ["semantic-repair", "review", "list", tail @ ..] => {
            let (limit, cursor) = parse_pagination_flags(tail)?;
            Ok(Command::SemanticRepairReviewList { limit, cursor })
        }
        ["semantic-repair", "review", "show", id] => Ok(Command::SemanticRepairReviewShow {
            id: nonempty(id, "semantic repair review ID")?,
        }),
        ["semantic-repair", "current", space_type, channel] => Ok(Command::SemanticRepairCurrent {
            space_type: nonempty(space_type, "space type")?,
            channel: nonempty(channel, "channel")?,
        }),
        ["semantic-repair", "generation", "build", tail @ ..] => {
            parse_semantic_repair_generation_build(tail)
        }
        ["semantic-repair", "generation", "list", tail @ ..] => {
            let (limit, cursor) = parse_pagination_flags(tail)?;
            Ok(Command::SemanticRepairGenerationList { limit, cursor })
        }
        ["semantic-repair", "generation", "show", id] => {
            Ok(Command::SemanticRepairGenerationShow {
                id: nonempty(id, "semantic repair generation ID")?,
            })
        }
        [
            "semantic-repair",
            "generation",
            "deploy",
            id,
            request,
            tail @ ..,
        ] => Ok(Command::SemanticRepairGenerationDeploy {
            id: nonempty(id, "semantic repair generation ID")?,
            request: nonempty(request, "signed semantic repair deployment intent")?,
            idempotency_key: parse_idempotency_flag(tail, "semantic-repair generation deploy")?,
        }),
        ["semantic-repair", "deployment", "current", space_type] => {
            Ok(Command::SemanticRepairDeploymentCurrent {
                space_type: nonempty(space_type, "space type")?,
            })
        }
        ["artifact", "custody", "plan", evidence_id, tail @ ..] => {
            let out = match tail {
                [] => None,
                ["--out", file] => Some(nonempty(file, "--out")?),
                other => return Err(unexpected(other)),
            };
            Ok(Command::ArtifactCustodyPlan {
                evidence_id: nonempty(evidence_id, "evidence ID")?,
                out,
            })
        }
        ["artifact", "custody", "create", evidence_id, tail @ ..] => {
            let mut cas_root = None;
            let mut out = None;
            let mut max_bytes = None;
            let mut iter = tail.iter();
            while let Some(flag) = iter.next() {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match *flag {
                    "--cas-root" => set_once(&mut cas_root, nonempty(value, "--cas-root")?, flag)?,
                    "--out" => set_once(&mut out, nonempty(value, "--out")?, flag)?,
                    "--max-bytes" => {
                        set_once(&mut max_bytes, positive_u64(value, "--max-bytes")?, flag)?
                    }
                    other => return Err(format!("unknown flag `{other}`")),
                }
            }
            Ok(Command::ArtifactCustodyCreate {
                evidence_id: nonempty(evidence_id, "evidence ID")?,
                cas_root: cas_root.ok_or("artifact custody create requires --cas-root DIR")?,
                out: out.ok_or("artifact custody create requires --out BUNDLE")?,
                max_bytes,
            })
        }
        ["artifact", "custody", "verify", bundle, tail @ ..] => {
            let (expected_plan_digest, tenant, incarnation, max_bytes, _, _) =
                parse_custody_identity_flags(tail, false)?;
            Ok(Command::ArtifactCustodyVerify {
                bundle: nonempty(bundle, "bundle")?,
                expected_plan_digest,
                tenant,
                incarnation,
                max_bytes,
            })
        }
        ["artifact", "custody", "restore", bundle, tail @ ..] => {
            let (expected_plan_digest, tenant, incarnation, max_bytes, cas_root, receipt) =
                parse_custody_identity_flags(tail, true)?;
            Ok(Command::ArtifactCustodyRestore {
                bundle: nonempty(bundle, "bundle")?,
                cas_root: cas_root.expect("required restore flag was checked"),
                expected_plan_digest,
                tenant,
                incarnation,
                receipt: receipt.expect("required restore flag was checked"),
                max_bytes,
            })
        }
        ["promotion", "status"] => Ok(Command::PromotionStatus),
        ["promotion", "recover"] => Ok(Command::PromotionRecover),
        ["promotion", "reconcile", space_type, channel, tail @ ..] => {
            let dry_run = match tail {
                [] => false,
                ["--dry-run"] => true,
                other => return Err(unexpected(other)),
            };
            Ok(Command::PromotionReconcile {
                space_type: nonempty(space_type, "space type")?,
                channel: nonempty(channel, "channel")?,
                dry_run,
            })
        }
        ["draft", space, file] => Ok(Command::Draft {
            space: space.to_string(),
            file: file.to_string(),
        }),
        ["cache", "stats"] => Ok(Command::CacheStats),
        ["cache", "clear"] => Ok(Command::CacheClear),
        ["login", username] => Ok(Command::Login {
            username: username.to_string(),
        }),
        other => Err(format!("unknown command: {} (try --help)", other.join(" "))),
    }
}

fn parse_semantic_repair_generation_build(tail: &[&str]) -> Result<Command, String> {
    let mut target_space = None;
    let mut channel = None;
    let mut expected_promotion_head_decision_id = None;
    let mut idempotency_key = None;
    let mut iter = tail.iter();
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match *flag {
            "--target-space" => {
                set_once(&mut target_space, nonempty(value, "--target-space")?, flag)?
            }
            "--channel" => set_once(&mut channel, nonempty(value, "--channel")?, flag)?,
            "--expected-promotion-head" => set_once(
                &mut expected_promotion_head_decision_id,
                lowercase_hex_record_id(value, "--expected-promotion-head")?,
                flag,
            )?,
            "--idempotency-key" => set_once(
                &mut idempotency_key,
                nonempty(value, "--idempotency-key")?,
                flag,
            )?,
            other => return Err(format!("unknown flag `{other}`")),
        }
    }
    Ok(Command::SemanticRepairGenerationBuild {
        target_space: target_space.ok_or_else(|| {
            "semantic-repair generation build requires --target-space SPACE_TYPE".to_string()
        })?,
        channel: channel.ok_or_else(|| {
            "semantic-repair generation build requires --channel CHANNEL".to_string()
        })?,
        expected_promotion_head_decision_id: expected_promotion_head_decision_id.ok_or_else(
            || {
                "semantic-repair generation build requires --expected-promotion-head DECISION_ID"
                    .to_string()
            },
        )?,
        idempotency_key: idempotency_key.ok_or_else(|| {
            "semantic-repair generation build requires --idempotency-key KEY".to_string()
        })?,
    })
}

fn lowercase_hex_record_id(value: &str, flag: &str) -> Result<String, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{flag} must be 64 lowercase hexadecimal characters"
        ));
    }
    Ok(value.to_string())
}

fn set_once<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        return Err(format!("{flag} may only be specified once"));
    }
    Ok(())
}

fn positive_u64(value: &str, flag: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{flag} expects a positive integer, got `{value}`"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be positive"));
    }
    Ok(parsed)
}

#[allow(clippy::type_complexity)]
fn parse_custody_identity_flags(
    tail: &[&str],
    restore: bool,
) -> Result<
    (
        String,
        String,
        String,
        Option<u64>,
        Option<String>,
        Option<String>,
    ),
    String,
> {
    let mut expected_plan_digest = None;
    let mut tenant = None;
    let mut incarnation = None;
    let mut max_bytes = None;
    let mut cas_root = None;
    let mut receipt = None;
    let mut iter = tail.iter();
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match *flag {
            "--expected-plan-digest" => set_once(
                &mut expected_plan_digest,
                nonempty(value, "--expected-plan-digest")?,
                flag,
            )?,
            "--tenant" => set_once(&mut tenant, nonempty(value, "--tenant")?, flag)?,
            "--incarnation" => set_once(&mut incarnation, nonempty(value, "--incarnation")?, flag)?,
            "--max-bytes" => set_once(&mut max_bytes, positive_u64(value, "--max-bytes")?, flag)?,
            "--cas-root" if restore => {
                set_once(&mut cas_root, nonempty(value, "--cas-root")?, flag)?
            }
            "--receipt" if restore => set_once(&mut receipt, nonempty(value, "--receipt")?, flag)?,
            other => return Err(format!("unknown flag `{other}`")),
        }
    }
    let operation = if restore { "restore" } else { "verify" };
    let expected_plan_digest = expected_plan_digest.ok_or_else(|| {
        format!("artifact custody {operation} requires --expected-plan-digest SHA256")
    })?;
    let tenant =
        tenant.ok_or_else(|| format!("artifact custody {operation} requires --tenant TENANT"))?;
    let incarnation = incarnation
        .ok_or_else(|| format!("artifact custody {operation} requires --incarnation ID"))?;
    if restore && cas_root.is_none() {
        return Err("artifact custody restore requires --cas-root DIR".into());
    }
    if restore && receipt.is_none() {
        return Err("artifact custody restore requires --receipt FILE".into());
    }
    Ok((
        expected_plan_digest,
        tenant,
        incarnation,
        max_bytes,
        cas_root,
        receipt,
    ))
}

/// Parse an optional trailing `--ttl-secs N` flag (token issue/rotate).
fn parse_ttl_flag(tail: &[&str]) -> Result<Option<u64>, String> {
    match tail {
        [] => Ok(None),
        ["--ttl-secs", value] => value
            .parse()
            .map(Some)
            .map_err(|_| format!("--ttl-secs expects a number, got `{value}`")),
        other => Err(format!("unexpected arguments: {}", other.join(" "))),
    }
}

fn parse_query(tail: &[&str]) -> Result<Command, String> {
    let mut write = false;
    let mut binds = Vec::new();
    let mut query = None;
    let mut iter = tail.iter();
    while let Some(arg) = iter.next() {
        match *arg {
            "--write" => write = true,
            "--bind" => {
                let pair = iter.next().ok_or("--bind requires NAME=JSON")?;
                let (name, raw) = pair
                    .split_once('=')
                    .ok_or_else(|| format!("--bind expects NAME=JSON, got `{pair}`"))?;
                // JSON value, falling back to a bare string for convenience
                // (`--bind cat=research` works without inner quotes).
                let value = serde_json::from_str(raw)
                    .unwrap_or_else(|_| serde_json::Value::String(raw.to_string()));
                binds.push((name.to_string(), value));
            }
            text if query.is_none() => query = Some(text.to_string()),
            other => return Err(format!("unexpected argument `{other}`")),
        }
    }
    Ok(Command::Query {
        query: query.ok_or("query text required")?,
        write,
        binds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(args: &[&str]) -> Invocation {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn connection_flags_and_commands() {
        let inv = parse_ok(&["--url", "http://h:1", "--token", "t", "health"]);
        assert_eq!(inv.url.as_deref(), Some("http://h:1"));
        assert_eq!(inv.token.as_deref(), Some("t"));
        assert_eq!(inv.command, Command::Health);

        assert_eq!(
            parse_ok(&["doc", "get", "c", "k"]).command,
            Command::DocGet {
                collection: "c".into(),
                key: "k".into()
            }
        );
        assert_eq!(
            parse_ok(&["doc", "list", "c", "--limit", "5"]).command,
            Command::DocList {
                collection: "c".into(),
                limit: Some(5),
                offset: None
            }
        );
        assert_eq!(
            parse_ok(&["export", "--out", "f.json"]).command,
            Command::Export {
                out: Some("f.json".into())
            }
        );
        assert_eq!(parse_ok(&[]).command, Command::Help);
    }

    #[test]
    fn query_flags_binds_and_json_fallback() {
        let Command::Query {
            query,
            write,
            binds,
        } = parse_ok(&[
            "query",
            "--write",
            "--bind",
            "n=5",
            "--bind",
            "cat=research",
            "FOR d IN c RETURN d",
        ])
        .command
        else {
            panic!("expected query");
        };
        assert!(write);
        assert_eq!(query, "FOR d IN c RETURN d");
        assert_eq!(binds[0], ("n".into(), serde_json::json!(5)));
        assert_eq!(binds[1], ("cat".into(), serde_json::json!("research")));
    }

    #[test]
    fn neuron_commands_parse() {
        assert_eq!(
            parse_ok(&["neuron", "list", "--space", "pharma"]).command,
            Command::NeuronList {
                space: Some("pharma".into()),
                status: None
            }
        );
        assert_eq!(
            parse_ok(&["neuron", "pending"]).command,
            Command::NeuronPending {
                space: None,
                limit: None
            }
        );
        assert_eq!(
            parse_ok(&["neuron", "pending", "pharma"]).command,
            Command::NeuronPending {
                space: Some("pharma".into()),
                limit: None,
            }
        );
        assert_eq!(
            parse_ok(&["neuron", "show", "k1"]).command,
            Command::NeuronShow { key: "k1".into() }
        );
        assert_eq!(
            parse_ok(&["neuron", "accept", "k1"]).command,
            Command::NeuronTransition {
                key: "k1".into(),
                action: "accept".into(),
                note: None
            }
        );
        assert_eq!(
            parse_ok(&["neuron", "graduation", "pharma"]).command,
            Command::NeuronGraduation {
                space: "pharma".into()
            }
        );
        assert!(parse(&["neuron", "explode", "k1"].map(String::from)).is_err());
    }

    #[test]
    fn errors_are_reported_not_panicked() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(parse(&args(&["--url"])).is_err());
        assert!(parse(&args(&["frobnicate"])).is_err());
        assert!(parse(&args(&["query", "--bind", "novalue", "q"])).is_err());
        assert!(parse(&args(&["doc", "list", "c", "--limit", "x"])).is_err());
    }

    #[test]
    fn job_commands_parse() {
        assert_eq!(
            parse_ok(&[
                "job",
                "submit",
                "construct.ingest",
                "@input.json",
                "--idempotency-key",
                "ingest-42",
            ])
            .command,
            Command::JobSubmit {
                kind: "construct.ingest".into(),
                input: "@input.json".into(),
                idempotency_key: "ingest-42".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "job",
                "list",
                "--kind",
                "construct.evaluate",
                "--status",
                "failed",
                "--limit",
                "20",
                "--offset",
                "5",
            ])
            .command,
            Command::JobList {
                kind: Some("construct.evaluate".into()),
                status: Some("failed".into()),
                limit: Some(20),
                cursor: None,
                archived: None,
                offset: Some(5),
            }
        );
        assert_eq!(
            parse_ok(&[
                "job",
                "list",
                "--cursor",
                "v1.abc_DEF-42~",
                "--archived",
                "include",
            ])
            .command,
            Command::JobList {
                kind: None,
                status: None,
                limit: None,
                cursor: Some("v1.abc_DEF-42~".into()),
                archived: Some("include".into()),
                offset: None,
            }
        );
        assert_eq!(
            parse_ok(&["job", "queue-status"]).command,
            Command::JobQueueStatus
        );
        assert_eq!(
            parse_ok(&[
                "job",
                "reconcile",
                "--dry-run",
                "--limit",
                "250",
                "--cursor",
                "next_page-1",
            ])
            .command,
            Command::JobReconcile {
                dry_run: true,
                limit: Some(250),
                cursor: Some("next_page-1".into()),
            }
        );
        assert_eq!(
            parse_ok(&[
                "job",
                "archive",
                "--before-ms",
                "1780000000000",
                "--limit",
                "100",
                "--dry-run",
                "--reason",
                "retention policy",
            ])
            .command,
            Command::JobArchive {
                before_ms: 1_780_000_000_000,
                limit: Some(100),
                dry_run: true,
                reason: Some("retention policy".into()),
            }
        );
        assert_eq!(
            parse_ok(&["job", "status", "job-1"]).command,
            Command::JobStatus { id: "job-1".into() }
        );
        assert_eq!(
            parse_ok(&["job", "cancel", "job-1", "--reason", "superseded"]).command,
            Command::JobCancel {
                id: "job-1".into(),
                reason: Some("superseded".into()),
            }
        );
        assert_eq!(
            parse_ok(&[
                "job",
                "retry",
                "job-1",
                "--reason",
                "transient",
                "--idempotency-key",
                "retry-1",
                "--mode",
                "restart",
            ])
            .command,
            Command::JobRetry {
                id: "job-1".into(),
                idempotency_key: "retry-1".into(),
                mode: Some("restart".into()),
                reason: Some("transient".into()),
            }
        );
    }

    #[test]
    fn job_idempotency_and_retry_mode_are_validated() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(parse(&args(&["job", "submit", "construct.ingest", "{}"])).is_err());
        assert!(parse(&args(&["job", "retry", "job-1"])).is_err());
        assert!(
            parse(&args(&[
                "job",
                "retry",
                "job-1",
                "--idempotency-key",
                "retry-1",
                "--mode",
                "rewind",
            ]))
            .is_err()
        );
        assert!(parse(&args(&["job", "list", "--cursor", "next&page",])).is_err());
        assert!(
            parse(&args(&[
                "job",
                "list",
                "--cursor",
                "next-page",
                "--offset",
                "10",
            ]))
            .is_err()
        );
        assert!(parse(&args(&["job", "list", "--archived", "yes"])).is_err());
        assert!(parse(&args(&["job", "archive", "--dry-run"])).is_err());
        assert!(parse(&args(&["job", "archive", "--before-ms", "yesterday"])).is_err());
        assert!(parse(&args(&["job", "reconcile", "--limit"])).is_err());
    }

    #[test]
    fn promotion_evidence_and_decision_queries_parse() {
        assert_eq!(
            parse_ok(&[
                "promotion",
                "evidence",
                "create",
                "@evidence.json",
                "--idempotency-key",
                "evidence-r1",
            ])
            .command,
            Command::PromotionEvidenceCreate {
                evidence: "@evidence.json".into(),
                idempotency_key: "evidence-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "promotion",
                "evidence",
                "list",
                "--limit",
                "25",
                "--cursor",
                "v1.next_page-2",
            ])
            .command,
            Command::PromotionEvidenceList {
                limit: Some(25),
                cursor: Some("v1.next_page-2".into()),
            }
        );
        assert_eq!(
            parse_ok(&["promotion", "evidence", "show", "evidence-1"]).command,
            Command::PromotionEvidenceShow {
                id: "evidence-1".into(),
            }
        );
        assert_eq!(
            parse_ok(&["promotion", "decisions", "--cursor", "v1.decision_2",]).command,
            Command::PromotionDecisions {
                limit: None,
                cursor: Some("v1.decision_2".into()),
            }
        );
        assert_eq!(
            parse_ok(&["promotion", "decision", "show", "decision-1"]).command,
            Command::PromotionDecisionShow {
                id: "decision-1".into(),
            }
        );
    }

    #[test]
    fn promotion_transitions_and_operator_commands_parse() {
        assert_eq!(
            parse_ok(&[
                "promotion",
                "promote",
                "evidence-2",
                "@promote-intent.json",
                "--idempotency-key",
                "promote-r1",
            ])
            .command,
            Command::PromotionPromote {
                evidence_id: "evidence-2".into(),
                intent: "@promote-intent.json".into(),
                idempotency_key: "promote-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "promotion",
                "promote",
                "evidence-1",
                r#"{"statement":{},"promoter_signature":{}}"#,
                "--idempotency-key",
                "promote-initial",
            ])
            .command,
            Command::PromotionPromote {
                evidence_id: "evidence-1".into(),
                intent: r#"{"statement":{},"promoter_signature":{}}"#.into(),
                idempotency_key: "promote-initial".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "promotion",
                "reject",
                "evidence-3",
                "@reject-intent.json",
                "--idempotency-key",
                "reject-r1",
            ])
            .command,
            Command::PromotionReject {
                evidence_id: "evidence-3".into(),
                intent: "@reject-intent.json".into(),
                idempotency_key: "reject-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&["promotion", "current", "clinical/pharma", "canary east"]).command,
            Command::PromotionCurrent {
                space_type: "clinical/pharma".into(),
                channel: "canary east".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "promotion",
                "rollback",
                "clinical/pharma",
                "canary east",
                "@rollback-intent.json",
                "--idempotency-key",
                "rollback-r1",
            ])
            .command,
            Command::PromotionRollback {
                space_type: "clinical/pharma".into(),
                channel: "canary east".into(),
                intent: "@rollback-intent.json".into(),
                idempotency_key: "rollback-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&["promotion", "status"]).command,
            Command::PromotionStatus
        );
        assert_eq!(
            parse_ok(&["promotion", "recover"]).command,
            Command::PromotionRecover
        );
        assert_eq!(
            parse_ok(&[
                "promotion",
                "reconcile",
                "clinical/pharma",
                "canary east",
                "--dry-run",
            ])
            .command,
            Command::PromotionReconcile {
                space_type: "clinical/pharma".into(),
                channel: "canary east".into(),
                dry_run: true,
            }
        );
    }

    #[test]
    fn promotion_required_flags_and_pagination_are_validated() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(parse(&args(&["promotion", "evidence", "create", "{}"])).is_err());
        assert!(parse(&args(&["promotion", "promote", "evidence-1"])).is_err());
        assert!(
            parse(&args(&[
                "promotion",
                "promote",
                "evidence-1",
                "@intent.json",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "promotion",
                "reject",
                "evidence-1",
                "@intent.json",
                "--idempotency-key",
                "   ",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "promotion",
                "rollback",
                "space",
                "stable",
                "@intent.json",
                "--idempotency-key",
                "",
            ]))
            .is_err()
        );
        assert!(parse(&args(&["promotion", "decisions", "--cursor", "not&safe",])).is_err());
        assert!(
            parse(&args(&[
                "promotion",
                "evidence",
                "list",
                "--limit",
                "10",
                "--limit",
                "20",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "promotion",
                "reconcile",
                "space",
                "stable",
                "--apply",
            ]))
            .is_err()
        );
    }

    #[test]
    fn governance_operator_commands_parse() {
        assert_eq!(
            parse_ok(&["governance", "status"]).command,
            Command::GovernanceStatus
        );
        assert_eq!(
            parse_ok(&[
                "governance",
                "key",
                "list",
                "--limit",
                "25",
                "--cursor",
                "v1.keys-2",
            ])
            .command,
            Command::GovernanceKeyList {
                limit: Some(25),
                cursor: Some("v1.keys-2".into()),
            }
        );
        assert_eq!(
            parse_ok(&[
                "governance",
                "key",
                "register",
                "@registration.json",
                "--idempotency-key",
                "register-r1",
            ])
            .command,
            Command::GovernanceKeyRegister {
                request: "@registration.json".into(),
                idempotency_key: "register-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&["governance", "key", "show", "registration-1"]).command,
            Command::GovernanceKeyShow {
                id: "registration-1".into()
            }
        );
        assert_eq!(
            parse_ok(&[
                "governance",
                "key",
                "revoke",
                "registration-1",
                "@revocation.json",
                "--idempotency-key",
                "revoke-r1",
            ])
            .command,
            Command::GovernanceKeyRevoke {
                registration_id: "registration-1".into(),
                request: "@revocation.json".into(),
                idempotency_key: "revoke-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&["governance", "revocation", "list", "--limit", "10",]).command,
            Command::GovernanceRevocationList {
                limit: Some(10),
                cursor: None,
            }
        );
        assert_eq!(
            parse_ok(&["governance", "revocation", "show", "revocation-1",]).command,
            Command::GovernanceRevocationShow {
                id: "revocation-1".into()
            }
        );
        assert_eq!(
            parse_ok(&["governance", "policy", "list"]).command,
            Command::GovernancePolicyList {
                limit: None,
                cursor: None,
            }
        );
        assert_eq!(
            parse_ok(&[
                "governance",
                "policy",
                "create",
                "@policy.json",
                "--idempotency-key",
                "policy-r1",
            ])
            .command,
            Command::GovernancePolicyCreate {
                request: "@policy.json".into(),
                idempotency_key: "policy-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&["governance", "policy", "show", "policy-1"]).command,
            Command::GovernancePolicyShow {
                id: "policy-1".into()
            }
        );
        assert_eq!(
            parse_ok(&[
                "governance",
                "policy",
                "approve",
                "policy-1",
                "@approval.json",
                "--idempotency-key",
                "approval-r1",
            ])
            .command,
            Command::GovernancePolicyApprove {
                id: "policy-1".into(),
                request: "@approval.json".into(),
                idempotency_key: "approval-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&["governance", "approval", "show", "approval-1"]).command,
            Command::GovernanceApprovalShow {
                id: "approval-1".into()
            }
        );
        assert_eq!(
            parse_ok(&["governance", "binding", "resolve", "approval-1",]).command,
            Command::GovernanceBindingResolve {
                approval_id: "approval-1".into()
            }
        );
        assert_eq!(
            parse_ok(&[
                "governance",
                "artifact",
                "attest",
                "@attestation.json",
                "--idempotency-key",
                "artifact-r1",
            ])
            .command,
            Command::GovernanceArtifactAttest {
                request: "@attestation.json".into(),
                idempotency_key: "artifact-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "governance",
                "artifact",
                "list",
                "--limit",
                "25",
                "--cursor",
                "scope-digest:aa:artifact-2",
            ])
            .command,
            Command::GovernanceArtifactList {
                limit: Some(25),
                cursor: Some("scope-digest:aa:artifact-2".into()),
            }
        );
        assert_eq!(
            parse_ok(&["governance", "artifact", "show", "attestation-1"]).command,
            Command::GovernanceArtifactShow {
                id: "attestation-1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "governance",
                "artifact",
                "binding",
                "resolve",
                "@binding.json",
            ])
            .command,
            Command::GovernanceArtifactBindingResolve {
                request: "@binding.json".into(),
            }
        );
    }

    #[test]
    fn governance_mutations_require_presigned_json_and_idempotency() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(parse(&args(&["governance", "key", "register"])).is_err());
        assert!(
            parse(&args(&[
                "governance",
                "key",
                "register",
                "@registration.json",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "governance",
                "policy",
                "approve",
                "policy-1",
                "@approval.json",
                "--idempotency-key",
                "",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "governance",
                "key",
                "list",
                "--cursor",
                "not&safe",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "governance",
                "artifact",
                "attest",
                "@attestation.json",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "governance",
                "artifact",
                "attest",
                "@attestation.json",
                "--idempotency-key",
                "",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "governance",
                "artifact",
                "list",
                "--cursor",
                "not&safe",
            ]))
            .is_err()
        );
    }

    #[test]
    fn semantic_repair_authority_commands_parse() {
        assert_eq!(
            parse_ok(&[
                "semantic-repair",
                "revision",
                "submit",
                "@revision.json",
                "--idempotency-key",
                "revision-r1",
            ])
            .command,
            Command::SemanticRepairRevisionSubmit {
                request: "@revision.json".into(),
                idempotency_key: "revision-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "semantic-repair",
                "revision",
                "list",
                "--limit",
                "8",
                "--cursor",
                "scope-digest:aa:revision-2",
            ])
            .command,
            Command::SemanticRepairRevisionList {
                limit: Some(8),
                cursor: Some("scope-digest:aa:revision-2".into()),
            }
        );
        assert_eq!(
            parse_ok(&["semantic-repair", "revision", "show", "revision-1"]).command,
            Command::SemanticRepairRevisionShow {
                id: "revision-1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "semantic-repair",
                "review",
                "submit",
                "revision-1",
                "@review.json",
                "--idempotency-key",
                "review-r1",
            ])
            .command,
            Command::SemanticRepairReviewSubmit {
                revision_id: "revision-1".into(),
                request: "@review.json".into(),
                idempotency_key: "review-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&["semantic-repair", "review", "list"]).command,
            Command::SemanticRepairReviewList {
                limit: None,
                cursor: None,
            }
        );
        assert_eq!(
            parse_ok(&["semantic-repair", "review", "show", "review-1"]).command,
            Command::SemanticRepairReviewShow {
                id: "review-1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "semantic-repair",
                "current",
                "clinical/pharma",
                "canary east",
            ])
            .command,
            Command::SemanticRepairCurrent {
                space_type: "clinical/pharma".into(),
                channel: "canary east".into(),
            }
        );
    }

    #[test]
    fn semantic_repair_authority_requires_complete_safe_arguments() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(
            parse(&args(&[
                "semantic-repair",
                "revision",
                "submit",
                "@revision.json",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "semantic-repair",
                "review",
                "submit",
                "revision-1",
                "@review.json",
                "--idempotency-key",
                "",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "semantic-repair",
                "revision",
                "list",
                "--cursor",
                "not&safe",
            ]))
            .is_err()
        );
        assert!(parse(&args(&["semantic-repair", "current", "space"])).is_err());
    }

    #[test]
    fn verified_semantic_repair_generation_commands_parse() {
        const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert_eq!(HEAD.len(), 64);
        assert_eq!(
            parse_ok(&[
                "semantic-repair",
                "generation",
                "build",
                "--channel",
                "canary east",
                "--expected-promotion-head",
                HEAD,
                "--target-space",
                "clinical/pharma",
                "--idempotency-key",
                "generation-r1",
            ])
            .command,
            Command::SemanticRepairGenerationBuild {
                target_space: "clinical/pharma".into(),
                channel: "canary east".into(),
                expected_promotion_head_decision_id: HEAD.into(),
                idempotency_key: "generation-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "semantic-repair",
                "generation",
                "list",
                "--limit",
                "12",
                "--cursor",
                "v1.generation-2",
            ])
            .command,
            Command::SemanticRepairGenerationList {
                limit: Some(12),
                cursor: Some("v1.generation-2".into()),
            }
        );
        assert_eq!(
            parse_ok(&["semantic-repair", "generation", "show", "generation-1"]).command,
            Command::SemanticRepairGenerationShow {
                id: "generation-1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "semantic-repair",
                "generation",
                "deploy",
                "generation-1",
                "@deployment.json",
                "--idempotency-key",
                "deployment-r1",
            ])
            .command,
            Command::SemanticRepairGenerationDeploy {
                id: "generation-1".into(),
                request: "@deployment.json".into(),
                idempotency_key: "deployment-r1".into(),
            }
        );
        assert_eq!(
            parse_ok(&[
                "semantic-repair",
                "deployment",
                "current",
                "clinical/pharma",
            ])
            .command,
            Command::SemanticRepairDeploymentCurrent {
                space_type: "clinical/pharma".into(),
            }
        );
    }

    #[test]
    fn verified_semantic_repair_generation_requires_pinned_safe_arguments() {
        const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(
            parse(&args(&[
                "semantic-repair",
                "generation",
                "build",
                "--target-space",
                "clinical",
                "--channel",
                "stable",
                "--idempotency-key",
                "generation-r1",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "semantic-repair",
                "generation",
                "build",
                "--target-space",
                "clinical",
                "--channel",
                "stable",
                "--expected-promotion-head",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "--idempotency-key",
                "generation-r1",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "semantic-repair",
                "generation",
                "build",
                "--target-space",
                "clinical",
                "--channel",
                "stable",
                "--expected-promotion-head",
                HEAD,
                "--expected-promotion-head",
                HEAD,
                "--idempotency-key",
                "generation-r1",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "semantic-repair",
                "generation",
                "deploy",
                "generation-1",
                "@deployment.json",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "semantic-repair",
                "generation",
                "list",
                "--cursor",
                "not&safe",
            ]))
            .is_err()
        );
        assert!(parse(&args(&["semantic-repair", "deployment", "current"])).is_err());
    }

    #[test]
    fn artifact_custody_commands_require_pinned_scope_and_destinations() {
        assert_eq!(
            parse_ok(&[
                "artifact",
                "custody",
                "plan",
                "evidence-1",
                "--out",
                "plan.json",
            ])
            .command,
            Command::ArtifactCustodyPlan {
                evidence_id: "evidence-1".into(),
                out: Some("plan.json".into()),
            }
        );
        assert_eq!(
            parse_ok(&[
                "artifact",
                "custody",
                "create",
                "evidence-1",
                "--cas-root",
                "/srv/cas",
                "--out",
                "/backup/bundle",
                "--max-bytes",
                "4096",
            ])
            .command,
            Command::ArtifactCustodyCreate {
                evidence_id: "evidence-1".into(),
                cas_root: "/srv/cas".into(),
                out: "/backup/bundle".into(),
                max_bytes: Some(4096),
            }
        );
        assert_eq!(
            parse_ok(&[
                "artifact",
                "custody",
                "verify",
                "/backup/bundle",
                "--expected-plan-digest",
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "--tenant",
                "acme",
                "--incarnation",
                "inc-1",
            ])
            .command,
            Command::ArtifactCustodyVerify {
                bundle: "/backup/bundle".into(),
                expected_plan_digest:
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                tenant: "acme".into(),
                incarnation: "inc-1".into(),
                max_bytes: None,
            }
        );
        assert_eq!(
            parse_ok(&[
                "artifact",
                "custody",
                "restore",
                "/backup/bundle",
                "--cas-root",
                "/srv/restored-cas",
                "--expected-plan-digest",
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "--tenant",
                "acme",
                "--incarnation",
                "inc-1",
                "--receipt",
                "restore.json",
            ])
            .command,
            Command::ArtifactCustodyRestore {
                bundle: "/backup/bundle".into(),
                cas_root: "/srv/restored-cas".into(),
                expected_plan_digest:
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
                tenant: "acme".into(),
                incarnation: "inc-1".into(),
                receipt: "restore.json".into(),
                max_bytes: None,
            }
        );

        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(
            parse(&args(&[
                "artifact",
                "custody",
                "create",
                "evidence-1",
                "--out",
                "bundle",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "artifact",
                "custody",
                "verify",
                "bundle",
                "--tenant",
                "acme",
                "--incarnation",
                "inc-1",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "artifact",
                "custody",
                "restore",
                "bundle",
                "--cas-root",
                "/srv/cas",
                "--expected-plan-digest",
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "--tenant",
                "acme",
                "--incarnation",
                "inc-1",
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "artifact",
                "custody",
                "create",
                "evidence-1",
                "--cas-root",
                "/srv/cas",
                "--out",
                "bundle",
                "--max-bytes",
                "0",
            ]))
            .is_err()
        );
    }
}
