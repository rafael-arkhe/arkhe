# VerifyBlocks.Tests.ps1 — testes Pester 3 (Windows PowerShell 5.1).
#
# Contrato canonico v582.0 (bloco 1075): hash = hex string de 64 caracteres,
# identico nos tres artefactos. Valida os 4 cenarios do gerador + a deteccao
# de HashReuse e DuplicateHash (as limitacoes que o PowerShell antigo nao
# cobria). Pester 3 nao permite Describe aninhado — um unico Describe.

Describe "verify-blocks.ps1 (canonical hex)" {

    BeforeAll {
        $Script = Join-Path (Join-Path $PSScriptRoot "..") "verify-blocks.ps1"
        $Generator = Join-Path (Join-Path $PSScriptRoot "..") "generate_test_blocks.py"
        $Base = Join-Path $env:TEMP "arkhe_verify_$(New-Guid)"
        New-Item -ItemType Directory -Path $Base -Force | Out-Null
        & python $Generator --base $Base 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "gerador falhou (exit $LASTEXITCODE)" }
    }

    AfterAll {
        Remove-Item -Recurse -Force $Base -ErrorAction SilentlyContinue
    }

    It "PASS em cadeia valida (exit 0)" {
        $null = & $Script -BlocksDir "$Base/valid" 2>&1
        $LASTEXITCODE | Should Be 0
    }

    It "FAIL em colisao de numero (exit 1)" {
        $null = & $Script -BlocksDir "$Base/collision" 2>&1
        $LASTEXITCODE | Should Be 1
    }

    It "FAIL em orfao (exit 1)" {
        $null = & $Script -BlocksDir "$Base/orphan" 2>&1
        $LASTEXITCODE | Should Be 1
    }

    It "FAIL em JSON invalido (exit 1)" {
        $null = & $Script -BlocksDir "$Base/invalid" 2>&1
        $LASTEXITCODE | Should Be 1
    }

    It "exit 2 para diretorio inexistente" {
        $null = & $Script -BlocksDir "$Base/nao_existe" 2>&1
        $LASTEXITCODE | Should Be 2
    }

    It "bug corrigido: collision produz dois ficheiros com tipos distintos" {
        $files = @(Get-ChildItem -Path "$Base/collision" -Filter "bloco_*.json")
        $files.Count | Should Be 2
        $tipos = @($files | ForEach-Object {
            (Get-Content $_.FullName -Raw | ConvertFrom-Json).tipo
        } | Select-Object -Unique)
        $tipos.Count | Should Be 2
    }

    It "hash e hex string de 64 caracteres (formato canonico)" {
        $sample = Get-Content "$Base/valid/bloco_0001.json" -Raw | ConvertFrom-Json
        ($sample.hash -match '^[0-9a-f]{64}$') | Should Be $true
    }

    It "encadeia 1->2->3 no cenario valid" {
        $b1 = Get-Content "$Base/valid/bloco_0001.json" -Raw | ConvertFrom-Json
        $b2 = Get-Content "$Base/valid/bloco_0002.json" -Raw | ConvertFrom-Json
        $b3 = Get-Content "$Base/valid/bloco_0003.json" -Raw | ConvertFrom-Json
        $b2.parent_hash | Should Be $b1.hash
        $b3.parent_hash | Should Be $b2.hash
    }

    It "detecta HashReuse (mesmo hash, numeros diferentes) (exit 1)" {
        $reuse = Join-Path $Base "reuse"
        New-Item -ItemType Directory -Path $reuse -Force | Out-Null
        $sameHash = "aa" * 32
        "{`"numero`":1,`"tipo`":`"A`",`"hash`":`"$sameHash`"}" | Set-Content "$reuse/bloco_0001.json" -Encoding UTF8
        "{`"numero`":2,`"tipo`":`"B`",`"hash`":`"$sameHash`"}" | Set-Content "$reuse/bloco_0002.json" -Encoding UTF8
        $output = & $Script -BlocksDir $reuse 2>&1
        $LASTEXITCODE | Should Be 1
        ($output -join "`n") | Should Match "HashReuse"
    }

    It "detecta DuplicateHash (mesmo numero/tipo, hash diferente) (exit 1)" {
        $dup = Join-Path $Base "dup"
        New-Item -ItemType Directory -Path $dup -Force | Out-Null
        "`{`"numero`":1,`"tipo`":`"A`",`"hash`":`"$('bb' * 32)`"`}" | Set-Content "$dup/bloco_0001.json" -Encoding UTF8
        "`{`"numero`":1,`"tipo`":`"A`",`"hash`":`"$('cc' * 32)`"`}" | Set-Content "$dup/bloco_0001_b.json" -Encoding UTF8
        $output = & $Script -BlocksDir $dup 2>&1
        $LASTEXITCODE | Should Be 1
        ($output -join "`n") | Should Match "DuplicateHash"
    }
}