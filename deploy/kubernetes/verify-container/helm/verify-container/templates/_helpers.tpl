{{- define "verify-container.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "verify-container.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- include "verify-container.name" . -}}
{{- end -}}
{{- end -}}

{{- define "verify-container.labels" -}}
app.kubernetes.io/name: {{ include "verify-container.fullname" . }}
app.kubernetes.io/component: verify-service
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" }}
{{- end -}}

{{- define "verify-container.selectorLabels" -}}
app.kubernetes.io/name: {{ include "verify-container.fullname" . }}
app.kubernetes.io/component: verify-service
{{- end -}}
