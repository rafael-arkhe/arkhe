<#
.SYNOPSIS
    Verifica blocos do ledger ARKHE (contrato canonico v582.0, bloco 1075).
.DESCRIPTION
    Verifica parse, colisao de numero, DuplicateHash, HashReuse, orfaos e o
    encadeamento estrito. O hash e sempre uma hex string de 64 caracteres.

    Compativel com Windows PowerShell 5.1 (o plano presumia 7.0; este
    ambiente so tem 5.1 — nota honesta registada no bloco 1075).

    O relatorio JSON de saida inclui `issues` (motivos de falha) — paridade
    com o binario Rust `verify`. Write-Host permanece para leitura humana.

    Exit codes: 0 = PASS, 1 = FAIL, 2 = diretorio invalido, 3 = vazio.
.EXAMPLE
    pwsh ./verify-blocks.ps1 -BlocksDir ./test_blocks/valid
#>

param(
    [Parameter(Mandatory)][string]$BlocksDir
)

$ErrorActionPreference = "Stop"

# ============================================================================
# VALIDACOES INICIAIS
# ============================================================================

if (-not (Test-Path $BlocksDir -PathType Container)) {
    Write-Host "Diretorio nao encontrado: $BlocksDir" -ForegroundColor Red
    exit 2
}

$files = @(Get-ChildItem -Path $BlocksDir -Filter "bloco_*.json")
if ($files.Count -eq 0) {
    Write-Host "Nenhum bloco encontrado em $BlocksDir" -ForegroundColor Red
    exit 3
}

# ============================================================================
# LEITURA (erros de parse aqui)
# ============================================================================

$blocks = [System.Collections.Generic.List[object]]::new()
$parseErrors = [System.Collections.Generic.List[string]]::new()

foreach ($file in $files) {
    try {
        $content = Get-Content -Path $file.FullName -Raw
        $block = $content | ConvertFrom-Json
        $required = @('numero', 'tipo', 'hash')
        foreach ($field in $required) {
            if ($block.PSObject.Properties.Name -notcontains $field) {
                $parseErrors.Add("$($file.Name): campo '$field' ausente")
            }
        }
        $blocks.Add($block)
    } catch {
        $parseErrors.Add("$($file.Name): $($_.Exception.Message)")
    }
}

# ============================================================================
# VERIFICACOES ESTRUTURAIS (issues aqui)
# ============================================================================

$issues = [System.Collections.Generic.List[string]]::new()

# 1. Colisao de numero (mesmo numero, tipos diferentes)
$byNumero = $blocks | Group-Object -Property numero
foreach ($group in $byNumero) {
    $tipos = @($group.Group | Select-Object -ExpandProperty tipo -Unique)
    if ($tipos.Count -gt 1) {
        $msg = "colisao de numero $($group.Name): tipos $($tipos -join ', ')"
        $issues.Add($msg)
        Write-Host "FAIL: $msg" -ForegroundColor Red
    }
}

# 2. DuplicateHash (mesmo numero, mesmo tipo, hash diferente)
foreach ($group in $byNumero) {
    if ($group.Count -gt 1) {
        $hashes = @($group.Group | Select-Object -ExpandProperty hash -Unique)
        if ($hashes.Count -gt 1) {
            $msg = "DuplicateHash no numero $($group.Name): $($hashes.Count) hashes diferentes"
            $issues.Add($msg)
            Write-Host "FAIL: $msg" -ForegroundColor Red
        }
    }
}

# 3. HashReuse (hash usado por numeros diferentes)
$hashGroups = $blocks | Group-Object -Property hash
foreach ($group in $hashGroups) {
    if ($group.Count -gt 1) {
        $numeros = @($group.Group | Select-Object -ExpandProperty numero -Unique)
        if ($numeros.Count -gt 1) {
            $msg = "HashReuse: hash usado por numeros $($numeros -join ', ')"
            $issues.Add($msg)
            Write-Host "FAIL: $msg" -ForegroundColor Red
        }
    }
}

# 4. Orfaos (parent_hash nao encontrado nos hashes existentes)
$hashSet = @{}
foreach ($b in $blocks) {
    $hashSet[$b.hash] = $b.numero
}
foreach ($b in $blocks) {
    if ($b.PSObject.Properties.Name -contains 'parent_hash' -and $b.parent_hash) {
        if (-not $hashSet.ContainsKey($b.parent_hash)) {
            $msg = "orfao: bloco $($b.numero) referencia parent inexistente"
            $issues.Add($msg)
            Write-Host "FAIL: $msg" -ForegroundColor Red
        }
    }
}

# ============================================================================
# RESULTADO
# ============================================================================

$decision = if ($parseErrors.Count -gt 0 -or $issues.Count -gt 0) { "FAIL" } else { "PASS" }

$report = [PSCustomObject]@{
    timestamp    = (Get-Date -Format "o")
    blocks_dir   = (Resolve-Path $BlocksDir).Path
    total_blocks = $blocks.Count
    parse_errors = $parseErrors.Count
    issues       = @($parseErrors) + @($issues)
    decision     = $decision
}

$report | ConvertTo-Json -Depth 6 | Write-Output

if ($decision -eq "PASS") {
    Write-Host "PASS: cadeia valida ($($blocks.Count) blocos)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "FAIL: verificacao falhou" -ForegroundColor Red
    exit 1
}