{{/*
Expand the name of the chart.
*/}}
{{- define "satori.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "satori.fullname" -}}
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

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "satori.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "satori.labels" -}}
helm.sh/chart: {{ include "satori.chart" . }}
{{ include "satori.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "satori.selectorLabels" -}}
app.kubernetes.io/name: {{ include "satori.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "satori.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "satori.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Agent labels
*/}}
{{- define "satori.agent.labels" -}}
{{ include "satori.labels" . }}
app.kubernetes.io/component: agent
satori.io/camera: {{ .camera.name | quote }}
{{- end }}

{{/*
Agent selector labels
*/}}
{{- define "satori.agent.selectorLabels" -}}
{{ include "satori.selectorLabels" . }}
app.kubernetes.io/component: agent
satori.io/camera: {{ .camera.name | quote }}
{{- end }}

{{/*
Event processor labels
*/}}
{{- define "satori.eventProcessor.labels" -}}
{{ include "satori.labels" . }}
app.kubernetes.io/component: event-processor
{{- end }}

{{/*
Event processor selector labels
*/}}
{{- define "satori.eventProcessor.selectorLabels" -}}
{{ include "satori.selectorLabels" . }}
app.kubernetes.io/component: event-processor
{{- end }}

{{/*
Archiver labels
*/}}
{{- define "satori.archiver.labels" -}}
{{ include "satori.labels" . }}
app.kubernetes.io/component: archiver
satori.io/archiver: {{ .archiver.name | quote }}
{{- end }}

{{/*
Archiver selector labels
*/}}
{{- define "satori.archiver.selectorLabels" -}}
{{ include "satori.selectorLabels" . }}
app.kubernetes.io/component: archiver
satori.io/archiver: {{ .archiver.name | quote }}
{{- end }}

{{/*
Image name for a component
Usage: {{ include "satori.image" (dict "Chart" .Chart "Values" .Values "component" "satori-agent") }}
*/}}
{{- define "satori.image" -}}
{{- $registry := .Values.image.registry -}}
{{- $component := .component -}}
{{- $tag := .Values.image.tag | default .Chart.AppVersion -}}
{{- printf "%s/%s:%s" $registry $component $tag -}}
{{- end }}
