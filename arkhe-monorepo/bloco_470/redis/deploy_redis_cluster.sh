#!/bin/bash
# =============================================================================
# BLOCO 470 v15 — REDIS CLUSTER (R1)
# deploy_redis_cluster.sh
#
# Deploy do Redis Cluster (replication + Sentinel) no Kubernetes via Helm.
# Requer: kubectl autenticado, helm 3+.
# =============================================================================
set -euo pipefail

NAMESPACE="${NAMESPACE:-catedral-cache}"
CHART_VERSION="${CHART_VERSION:-}"
REDIS_PASSWORD="${REDIS_PASSWORD:-}"

if [ -z "$REDIS_PASSWORD" ]; then
    echo "❌ REDIS_PASSWORD é obrigatória (export antes de executar)." >&2
    exit 1
fi

# 1. Adicionar repositório Bitnami
echo "📦 Adicionando repositório Bitnami..."
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo update

# 2. Criar namespace
echo "🏷️  Criando namespace $NAMESPACE..."
kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -

# 3. Deploy Redis Cluster com Helm
HELM_ARGS=(upgrade --install redis-catedral bitnami/redis \
    --namespace "$NAMESPACE" \
    --values "$(dirname "$0")/redis-cluster-values.yaml" \
    --set auth.password="$REDIS_PASSWORD" \
    --set sentinel.master="mymaster" \
    --set replica.replicaCount=3 \
    --set sentinel.replicas=3 \
    --wait)

if [ -n "$CHART_VERSION" ]; then
    HELM_ARGS+=(--version "$CHART_VERSION")
fi

echo "🚀 Deploying Redis Cluster..."
helm "${HELM_ARGS[@]}"

# 4. Verificar deployment
echo ""
echo "📋 Pods:"
kubectl get pods -n "$NAMESPACE"
echo ""
echo "📋 Serviços:"
kubectl get svc -n "$NAMESPACE"

echo ""
echo "✅ Redis Cluster implantado com sucesso"
echo "   Master:   redis-catedral-master.$NAMESPACE.svc.cluster.local:6379"
echo "   Sentinel: redis-catedral-sentinel.$NAMESPACE.svc.cluster.local:26379"
echo ""
echo "   Canais de conexão (cliente Python):"
echo "   - REDIS_PASSWORD=$REDIS_PASSWORD"
echo "   - REDIS_SENTINEL_HOSTS=redis-catedral-sentinel:$NAMESPACE.svc.cluster.local:26379"