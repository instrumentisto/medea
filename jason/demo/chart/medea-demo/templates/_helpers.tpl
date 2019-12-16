{{/*
Expand the name of the chart.
*/}}
{{- define "medea-demo.name" -}}
{{- .Chart.Name | trunc 50 | trimSuffix "-" -}}
{{- end -}}

{{/*
Create a default fully qualified app name.
We truncate at 50 chars because some Kubernetes name fields are limited
to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "medea-demo.fullname" -}}
{{- $name := include "medea-demo.name" . -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 50 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 50 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "medea-demo.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 50 | trimSuffix "-" -}}
{{- end -}}
