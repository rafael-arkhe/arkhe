# =============================================================================
# BLOCO 470 v15 — DNSSEC (D1)
# dnssec.tf — Terraform para DNSSEC no AWS Route 53
#
# Aplica terraform:
#   terraform init && terraform plan && terraform apply
# =============================================================================

variable "domain" {
  description = "Domínio raiz da zona pública (ex.: catedral.os)."
  type        = string
}

variable "aws_region" {
  description = "Região AWS para a KMS key de assinatura."
  type        = string
  default     = "us-east-1"
}

# Zona pública existente (promovida para a zona usando DNSSEC)
data "aws_route53_zone" "main" {
  name         = var.domain
  private_zone = false
}

# --- KMS key para assinatura DNSSEC -------------------------------------
resource "aws_kms_key" "dnssec" {
  description             = "DNSSEC key for ${var.domain}"
  key_usage               = "SIGN_VERIFY"
  customer_master_key_spec = "ECC_NIST_P256"
  deletion_window_in_days = 7

  tags = {
    Name = "dnssec-${var.domain}"
  }
}

resource "aws_kms_alias" "dnssec" {
  name          = "alias/dnssec-${var.domain}"
  target_key_id = aws_kms_key.dnssec.key_id
}

# --- Key Signing Key (KSK) ----------------------------------------------
resource "aws_route53_key_signing_key" "dnssec" {
  hosted_zone_id = data.aws_route53_zone.main.id
  name           = "KSK-${var.domain}"
  kms_key_id     = aws_kms_key.dnssec.key_id
}

# --- Ativação do signing (após existir a KSK) ---------------------------
resource "aws_route53_hosted_zone_dnssec" "dnssec" {
  hosted_zone_id = aws_route53_key_signing_key.dnssec.hosted_zone_id
  signing_status = "SIGNING"

  depends_on = [aws_route53_key_signing_key.dnssec]
}

# --- DS records para registro no registrador ----------------------------
data "aws_route53_dnssec_record" "ds" {
  hosted_zone_id = aws_route53_hosted_zone_dnssec.dnssec.hosted_zone_id
}

# Output para registrar DS no registrador
output "ds_records" {
  description = "DS records a registrar no registrador do TLD."
  value = [
    for r in data.aws_route53_dnssec_record.ds.signing_key :
    "DS ${data.aws_route53_zone.main.name} ${r.key_tag} ${r.algorithm} ${r.digest_type} ${r.digest}"
  ]
}

output "kms_key_id" {
  description = "ID da KMS key utilizada para assinatura DNSSEC."
  value       = aws_kms_key.dnssec.key_id
}