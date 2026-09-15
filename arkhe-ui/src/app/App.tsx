import { type FormEvent, useCallback, useEffect, useState } from 'react';

import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Input,
} from '@arkhe/ui';

import { loadVerify, type AttestationReport, type VerifyApi } from './verify-adapter';

/** Os quatro estágios do pipeline, na ordem em que o crate os reporta. */
const STAGES = [
  { key: 'sha256', label: 'Digest SHA-256 do payload' },
  { key: 'signature', label: 'Assinatura Ed25519 contra o trust root' },
  { key: 'inclusion', label: 'Inclusão Merkle (RFC 6962)' },
  { key: 'quorum', label: 'Quórum de testemunhas distintas (≥ 2)' },
] as const;

/**
 * O estado do carregamento do WASM. Três estados explícitos em vez de
 * `api | null` + `error | null`: com dois `null` é possível representar
 * "carregado e falhado ao mesmo tempo", que não existe.
 */
type WasmState =
  | { kind: 'loading' }
  | { kind: 'ready'; api: VerifyApi }
  | { kind: 'failed'; message: string };

export function App() {
  const [wasm, setWasm] = useState<WasmState>({ kind: 'loading' });
  const [attestationJson, setAttestationJson] = useState('');
  const [trustRootJson, setTrustRootJson] = useState('');
  const [report, setReport] = useState<AttestationReport | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    loadVerify().then(
      (api) => {
        if (!cancelled) setWasm({ kind: 'ready', api });
      },
      (error: unknown) => {
        if (!cancelled) {
          setWasm({
            kind: 'failed',
            message: error instanceof Error ? error.message : String(error),
          });
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  const onSubmit = useCallback(
    (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (wasm.kind !== 'ready') return;
      try {
        setReport(wasm.api.verifyAttestation(attestationJson, trustRootJson));
        setFailure(null);
      } catch (error: unknown) {
        // O crate não lança sobre bytes não confiáveis; o que pode falhar aqui
        // é a interpretação do relatório — e isso é um bug, não um veredito.
        setReport(null);
        setFailure(error instanceof Error ? error.message : String(error));
      }
    },
    [wasm, attestationJson, trustRootJson],
  );

  const canVerify = wasm.kind === 'ready' && attestationJson.trim() !== '';

  return (
    <main className="app">
      <header className="app__header">
        <h1 className="app__title">Verificação de atestações</h1>
        <p className="app__lede">
          O veredito é calculado localmente por um módulo WebAssembly derivado do crate{' '}
          <code className="app__code">arkhe-verify-wasm</code> (alvo{' '}
          <code className="app__code">web</code>). Nada do que se escreve abaixo é enviado a um
          servidor.
        </p>
        <div className="app__badge-row">
          <WasmStatusBadge state={wasm} />
        </div>
      </header>

      <Card>
        <form onSubmit={onSubmit}>
          <CardHeader>
            <CardTitle>Atestação e trust root</CardTitle>
            <CardDescription>
              O JSON da atestação e a lista de chaves públicas em que se confia (array de chaves em
              hex).
            </CardDescription>
          </CardHeader>
          <CardContent className="app-form__fields">
            <div className="app-form__field">
              <label htmlFor="attestation" className="app-form__label">
                Atestação (JSON)
              </label>
              <Input
                id="attestation"
                name="attestation"
                value={attestationJson}
                onChange={(event) => setAttestationJson(event.target.value)}
                placeholder='{"payload_b64":"…","merkle_root_hex":"…", …}'
                autoComplete="off"
                spellCheck={false}
              />
            </div>
            <div className="app-form__field">
              <label htmlFor="trust-root" className="app-form__label">
                Trust root (JSON)
              </label>
              <Input
                id="trust-root"
                name="trust-root"
                value={trustRootJson}
                onChange={(event) => setTrustRootJson(event.target.value)}
                placeholder='["1f8f…c2","a30b…77"]'
                autoComplete="off"
                spellCheck={false}
              />
            </div>
          </CardContent>
          <CardFooter>
            <Button type="submit" disabled={!canVerify}>
              Verificar
            </Button>
            {wasm.kind === 'loading' && <span className="app__hint">a carregar o verificador…</span>}
          </CardFooter>
        </form>
      </Card>

      {failure !== null && (
        <Card variant="surface" role="alert">
          <CardHeader>
            <CardTitle>Falha ao verificar</CardTitle>
            <CardDescription>
              Isto não é um veredito: o verificador não chegou a produzir um relatório.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <pre className="app-failure__detail">{failure}</pre>
          </CardContent>
        </Card>
      )}

      {report !== null && <ReportCard report={report} />}
    </main>
  );
}

function WasmStatusBadge({ state }: { state: WasmState }) {
  if (state.kind === 'loading') {
    return <Badge status="pending">verificador WASM: a carregar</Badge>;
  }
  if (state.kind === 'failed') {
    return <Badge status="error">verificador WASM: indisponível</Badge>;
  }
  return <Badge status="verified">verificador WASM: pronto</Badge>;
}

const VERDICTS = {
  verified: { status: 'verified', label: 'Verificada' },
  partial: { status: 'partial', label: 'Parcial' },
  error: { status: 'error', label: 'Rejeitada' },
} as const;

function ReportCard({ report }: { report: AttestationReport }) {
  const failed = STAGES.filter((stage) => !report[stage.key]).length;
  const verdict =
    report.ok ? VERDICTS.verified : failed === STAGES.length ? VERDICTS.error : VERDICTS.partial;

  return (
    <Card role="status" aria-live="polite">
      <CardHeader>
        <div className="app-result__heading">
          <CardTitle>Resultado</CardTitle>
          {/* O texto do badge carrega o significado; a cor só reforça. */}
          <Badge status={verdict.status}>{verdict.label}</Badge>
        </div>
        <CardDescription>
          {report.ok
            ? 'Os quatro estágios do pipeline concordam.'
            : `${failed} de ${STAGES.length} estágios falharam.`}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <ul className="app-result__stages">
          {STAGES.map((stage) => {
            const passed = report[stage.key];
            return (
              <li key={stage.key} className="app-result__stage">
                <span className="app-result__stage-label">{stage.label}</span>
                <Badge status={passed ? 'verified' : 'error'}>
                  {passed ? 'confere' : 'falha'}
                </Badge>
              </li>
            );
          })}
        </ul>
      </CardContent>
      {report.error !== null && (
        <CardFooter>
          <p className="app-result__cause">
            <span className="app-result__cause-term">Causa: </span>
            {report.error}
          </p>
        </CardFooter>
      )}
    </Card>
  );
}
