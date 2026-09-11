{{/*
Expand the name of the chart.
*/}}
{{- define "cognigraph.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "cognigraph.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "cognigraph.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "cognigraph.labels" -}}
helm.sh/chart: {{ include "cognigraph.chart" . }}
{{ include "cognigraph.selectorLabels" . }}
app.kubernetes.io/version: {{ include "cognigraph.imageTag" . | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "cognigraph.selectorLabels" -}}
app.kubernetes.io/name: {{ include "cognigraph.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "cognigraph.serverSelectorLabels" -}}
{{ include "cognigraph.selectorLabels" . }}
app.kubernetes.io/component: server
{{- end }}

{{- define "cognigraph.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "cognigraph.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{- define "cognigraph.imageTag" -}}
{{- $default := .Chart.AppVersion -}}
{{- if eq .Values.edition "enterprise" -}}
{{- $default = printf "%s-enterprise" .Chart.AppVersion -}}
{{- end -}}
{{- .Values.image.tag | default $default -}}
{{- end }}

{{- define "cognigraph.image" -}}
{{- printf "%s:%s" .Values.image.repository (include "cognigraph.imageTag" .) }}
{{- end }}

{{/*
Name of the Secret holding admin-password, host-admin-password, jwt-secret.
*/}}
{{- define "cognigraph.authSecretName" -}}
{{- if .Values.auth.existingSecret }}
{{- .Values.auth.existingSecret }}
{{- else }}
{{- printf "%s-auth" (include "cognigraph.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Validate the values that must not be wrong silently.
*/}}
{{- define "cognigraph.validate" -}}
{{- if not (has .Values.edition (list "community" "enterprise")) }}
{{- fail "cognigraph: edition must be community or enterprise." }}
{{- end }}
{{- if and (eq .Values.edition "community") (or .Values.enterprise.multiTenant .Values.enterprise.governanceRootPublicKey .Values.enterprise.artifactCas.existingClaim) }}
{{- fail "cognigraph: enterprise_feature_required: these settings require edition=enterprise and an Enterprise image." }}
{{- end }}
{{- range $key, $_ := .Values.podLabels }}
{{- if or (hasPrefix "app.kubernetes.io/" $key) (eq $key "helm.sh/chart") }}
{{- fail "cognigraph: podLabels must not override chart identity labels." }}
{{- end }}
{{- end }}
{{- if and .Values.backup.enabled (not (regexMatch "^[1-9][0-9]*$" (toString .Values.backup.retentionDays))) }}
{{- fail "cognigraph: backup.retentionDays must be a positive integer." }}
{{- end }}
{{- if ne (toString .Values.replicaCount) "1" }}
{{- fail "cognigraph: replicaCount must be 1. The Native backend is a single-writer store; read replicas are not part of this chart yet (see docs/architecture/design-notes/read-replicas.md)." }}
{{- end }}
{{- if and .Values.auth.enabled (not .Values.auth.existingSecret) (or (not .Values.auth.adminPassword) (not .Values.auth.hostAdminPassword) (not .Values.auth.jwtSecret)) }}
{{- fail "cognigraph: auth.enabled=true requires auth.adminPassword, auth.hostAdminPassword and auth.jwtSecret, or auth.existingSecret with keys admin-password, host-admin-password, jwt-secret." }}
{{- end }}
{{- if and (eq .Values.storage.mode "paged") (ne .Values.storage.vectorMode "sidecar") }}
{{- fail "cognigraph: storage.mode=paged requires storage.vectorMode=sidecar." }}
{{- end }}
{{- if and .Values.backup.enabled (not .Values.auth.enabled) }}
{{- fail "cognigraph: backup.enabled=true requires auth.enabled=true (GET /api/admin/export is an admin route)." }}
{{- end }}
{{- end }}
