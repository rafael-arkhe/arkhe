# Substrato 212 — Certificate Gateway (Standalone v5.1)

## Descrição

Pacote autossuficiente para gestão de certificados, JWT, Vault, CT Logs e validação ANATEL.

## Alterações na v5.1

- **Correção:** `cryptography.__version__` agora é obtido corretamente (não mais `x509.__version__`).
- **Unificação:** As faixas ANATEL são importadas do módulo canônico `anatel_band_guard.py`
  (Substrato 227), garantindo consistência com o restante da Catedral OS. O caminho do
  diretório pai é adicionado ao `sys.path` para que a importação resolva
  independentemente do diretório de trabalho.
- **Testes:** Alinhados com a semântica canônica das mensagens de restrição.

## Requisitos

- Python 3.12+ (testado: 3.14)
- pip

## Instalação

```bash
pip install -r requirements.txt
```

## Execução

```bash
python substrate_212.py --status
python substrate_212.py --issue-jwt "arquitect" "admin"
python substrate_212.py --verify-jwt "eyJhbGciOiJSUzI1Ni..."
python substrate_212.py --cert "meu-dominio.local"
python substrate_212.py --ct-logs "github.com"
python substrate_212.py --freq 915.0
```

## Testes

```bash
pytest test_substrate_212.py -v
```

## Variáveis de Ambiente

- `CATEDRAL_RSA_PRIVATE_KEY`: PEM da chave RSA (opcional)
- `VAULT_ADDR`: URL do Vault (ex: https://vault:8200)
- `VAULT_TOKEN`: Token de autenticação do Vault

## Notas de honestidade (auditoria Arquiteto-Ω)

- **CT Logs** e **Vault** só reportam sucesso quando há serviço/rede vivos;
  caso contrário relatam indisponível — nunca sucesso falso.
- **ANATEL unificado**: se `anatel_band_guard.py` não estiver disponível, o
  standalone usa um fallback inline com a mesma semântica. O comportamento
  real é verificado pelos testes.

## Licença

MIT
