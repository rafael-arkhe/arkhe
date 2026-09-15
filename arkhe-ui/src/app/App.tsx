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

import {
  inspectGgufModel,
  nativeInspectionAvailable,
  type InspectionResult,
  type ModelInspection,
} from './gguf-adapter';
import {
  checkForUpdate,
  updaterRuntimeAvailable,
  type UpdateCheck,
  type UpdateInfo,
} from './updater-adapter';
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

  // O núcleo nativo é uma capacidade do ambiente, não um estado que a app
  // controla: pergunta-se uma vez, no primeiro render, e a resposta não muda
  // durante a vida da página.
  const [nativeAvailable] = useState(nativeInspectionAvailable);
  const [modelPath, setModelPath] = useState('');
  const [inspection, setInspection] = useState<InspectionResult | null>(null);
  const [inspecting, setInspecting] = useState(false);

  // O mesmo critério para o updater: é uma capacidade do ambiente. Lido à parte
  // do `nativeAvailable` porque são fronteiras diferentes — o updater é um
  // plugin de terceiros e esta fase só consulta; não descarrega nem instala.
  const [updaterAvailable] = useState(updaterRuntimeAvailable);
  const [updateCheck, setUpdateCheck] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);

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

  /**
   * O botão fica desactivado durante o pedido, portanto há no máximo um em voo —
   * e não é preciso correlacionar respostas fora de ordem pelo `path` que o
   * comando ecoa. (O eco continua a valer: mostra *que* ficheiro foi lido.)
   */
  const onInspect = useCallback(
    (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (!nativeAvailable || modelPath.trim() === '' || inspecting) return;

      setInspecting(true);
      // `inspectGgufModel` nunca lança: as três saídas vêm no resultado.
      inspectGgufModel(modelPath).then((result) => {
        setInspection(result);
        setInspecting(false);
      });
    },
    [nativeAvailable, modelPath, inspecting],
  );

  /**
   * A consulta ao plugin de actualização.
   *
   * O botão fica desactivado durante o pedido, portanto há no máximo um em voo —
   * a mesma razão do formulário do modelo acima. E `checkForUpdate` **nunca
   * lança**: as cinco saídas vêm no resultado, incluindo a de "não há runtime
   * Tauri", que é o que um `dist-app` servido no navegador recebe.
   */
  const onCheckUpdate = useCallback(() => {
    if (checking) return;
    setChecking(true);
    checkForUpdate().then((result) => {
      setUpdateCheck(result);
      setChecking(false);
    });
  }, [checking]);

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

      <Card>
        <form onSubmit={onInspect}>
          <CardHeader>
            <div className="app-result__heading">
              <CardTitle>Modelo GGUF — núcleo nativo</CardTitle>
              <Badge status={nativeAvailable ? 'verified' : 'pending'}>
                {nativeAvailable ? 'núcleo nativo: disponível' : 'núcleo nativo: indisponível'}
              </Badge>
            </div>
            <CardDescription>
              Lê o arquivo <em>no processo Rust</em> e devolve o SHA-256 e o cabeçalho dos{' '}
              <strong>mesmos</strong> bytes, pelo comando{' '}
              <code className="app__code">inspect_gguf_model</code>. Os bytes não atravessam o
              IPC — é por isso que esta rota existe ao lado da do WASM.
            </CardDescription>
          </CardHeader>
          <CardContent className="app-form__fields">
            <div className="app-form__field">
              <label htmlFor="model-path" className="app-form__label">
                Caminho do modelo
              </label>
              <Input
                id="model-path"
                name="model-path"
                value={modelPath}
                onChange={(event) => setModelPath(event.target.value)}
                placeholder="C:\modelos\modelo.gguf"
                autoComplete="off"
                spellCheck={false}
              />
            </div>
            {/*
              Degradação honesta: fora do Tauri não há processo nativo. Diz-se o
              que não está disponível e o que continua a funcionar, em vez de
              esconder o formulário ou falhar em silêncio.
            */}
            {!nativeAvailable && (
              <p className="app__hint" role="note">
                Não há processo nativo a atender: esta página está a correr fora do Tauri (no
                navegador), onde não existe acesso ao sistema de ficheiros. A verificação de
                atestações acima continua a funcionar — é WASM. Para inspeccionar um modelo,
                correr <code className="app__code">npm run tauri:dev</code>.
              </p>
            )}
          </CardContent>
          <CardFooter>
            <Button
              type="submit"
              disabled={!nativeAvailable || modelPath.trim() === '' || inspecting}
            >
              Inspecionar
            </Button>
            {inspecting && <span className="app__hint">a ler o arquivo…</span>}
          </CardFooter>
        </form>
      </Card>

      {inspection !== null && <InspectionCard result={inspection} />}

      <Card>
        <CardHeader>
          <div className="app-result__heading">
            <CardTitle>Actualizações da aplicação</CardTitle>
            <Badge status={updaterAvailable ? 'verified' : 'pending'}>
              {updaterAvailable ? 'runtime: disponível' : 'runtime: indisponível'}
            </Badge>
          </div>
          <CardDescription>
            Resolve a referência de release publicada em hashtree e diz se há versão mais recente
            para esta plataforma, pelo comando{' '}
            <code className="app__code">plugin:hashtree-updater|check</code>. Este controlo{' '}
            <strong>só lê o manifesto</strong>: não descarrega nem substitui nada.
          </CardDescription>
        </CardHeader>
        {/*
          O botão continua activo fora do Tauri, ao contrário do de inspecção.
          É deliberado: se estivesse desactivado, o estado "indisponível fora do
          Tauri" nunca seria mostrado — e é um dos desfechos que a interface tem
          de saber dizer. Carregar aqui é o que o torna observável no navegador.
        */}
        {!updaterAvailable && (
          <CardContent>
            <p className="app__hint" role="note">
              Não há processo nativo a atender o IPC: esta página está a correr fora do Tauri (no
              navegador), onde não existe o runtime que fala com os relays. O botão funciona na
              mesma e diz isso mesmo — não é um erro de rede nem um "não há novidade". Para
              consultar a sério, correr <code className="app__code">npm run tauri:dev</code>.
            </p>
          </CardContent>
        )}
        <CardFooter>
          <Button onClick={onCheckUpdate} disabled={checking}>
            Procurar actualizações
          </Button>
          {checking && <span className="app__hint">a ler o manifesto publicado…</span>}
        </CardFooter>
      </Card>

      {updateCheck !== null && <UpdateResultCard result={updateCheck} />}
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

/**
 * O resultado de uma inspecção nativa — os três casos do [`InspectionResult`].
 *
 * `unavailable` não devia ser alcançável (o botão está desactivado quando não há
 * runtime), mas está tratado por inteiro de propósito: um caso por omissão numa
 * união é exactamente o sítio onde um estado impossível se torna um ecrã em
 * branco. Aqui, se acontecer, diz-se o que aconteceu — e diz-se que **não** é
 * uma falha de leitura, porque não é.
 */
function InspectionCard({ result }: { result: InspectionResult }) {
  if (result.kind === 'unavailable') {
    return (
      <Card variant="surface" role="status">
        <CardHeader>
          <CardTitle>Inspecção indisponível</CardTitle>
          <CardDescription>
            O pedido foi feito sem um runtime Tauri, portanto não houve processo nativo que
            pudesse ler o arquivo. Isto não é um erro de leitura nem um veredito sobre o
            modelo.
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  if (result.kind === 'failed') {
    return (
      <Card variant="surface" role="alert">
        <CardHeader>
          <CardTitle>Inspecção não produziu resultado</CardTitle>
          <CardDescription>
            Isto não é um veredito sobre o modelo: o comando não chegou a devolver uma
            inspecção.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <pre className="app-failure__detail">{result.message}</pre>
        </CardContent>
      </Card>
    );
  }

  return <InspectionReportCard inspection={result.inspection} />;
}

/** `null` é "o byte não estava lá", não zero — e escreve-se assim. */
function ausenteOu(value: number | null): string {
  return value === null ? 'ausente' : String(value);
}

/**
 * O relatório, com os quatro factos **separados**.
 *
 * O digest e o cabeçalho aparecem lado a lado mesmo quando um deles falha: um
 * ficheiro truncado tem um digest perfeitamente calculável e um cabeçalho que
 * não interpreta, e esconder o digest por causa do cabeçalho esconderia o único
 * dado que existe.
 */
function InspectionReportCard({ inspection }: { inspection: ModelInspection }) {
  const { header } = inspection;

  return (
    <Card role="status" aria-live="polite">
      <CardHeader>
        <div className="app-result__heading">
          <CardTitle>Inspecção do modelo</CardTitle>
          <Badge status={header.ok ? 'verified' : 'error'}>
            {header.ok ? 'cabeçalho GGUF válido' : 'cabeçalho recusado'}
          </Badge>
        </div>
        <CardDescription>
          Lido de <code className="app__code">{inspection.path}</code>
        </CardDescription>
      </CardHeader>
      <CardContent>
        <dl className="app-inspect__facts">
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Digest SHA-256 do modelo</dt>
            <dd className="app-inspect__fact-value">{inspection.digest_hex}</dd>
          </div>
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Bytes entregues ao parser</dt>
            <dd className="app-inspect__fact-value">{inspection.bytes}</dd>
          </div>
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Magic `GGUF`</dt>
            <dd className="app-inspect__fact-value">
              {header.magic_ok ? 'confere' : 'não confere'}
            </dd>
          </div>
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Versão declarada</dt>
            <dd className="app-inspect__fact-value">{ausenteOu(header.version)}</dd>
          </div>
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Tensores</dt>
            <dd className="app-inspect__fact-value">{ausenteOu(header.tensor_count)}</dd>
          </div>
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Pares chave-valor</dt>
            <dd className="app-inspect__fact-value">{ausenteOu(header.metadata_kv_count)}</dd>
          </div>
        </dl>
      </CardContent>
      {header.error !== null && (
        <CardFooter>
          <p className="app-result__cause">
            <span className="app-result__cause-term">Causa: </span>
            {header.error}
          </p>
        </CardFooter>
      )}
    </Card>
  );
}

/**
 * O resultado da consulta de actualizações — os cinco casos do
 * [`UpdateCheck`](./updater-adapter.ts), cada um com o **seu próprio** texto.
 *
 * A separação que importa é entre `failed` e os dois "não há novidade"
 * (`current` e `no-release`). Com os relays a não responder, o `check()`
 * rejeita; apresentar isso como "está actualizado" seria trocar "não consegui
 * perguntar" por "confirmei que não há nada" — a única resposta que este ecrã
 * não pode dar. Os cartões de erro usam `role="alert"` e os restantes
 * `role="status"`, como os outros resultados da app.
 *
 * `no-release` tem cartão próprio por uma segunda razão, mais fina: o plugin
 * devolve `Ok(None)` tanto para "nada publicado" como para "nenhum asset serve
 * este alvo", e o README dele recomenda tratá-lo como "sem novidade" e ficar
 * calado — bom conselho para um banner automático, insuficiente para uma
 * consulta que o utilizador pediu. Aqui diz-se o que se sabe: não houve nada
 * aplicável, e portanto não houve comparação de versões nenhuma.
 */
function UpdateResultCard({ result }: { result: UpdateCheck }) {
  if (result.kind === 'failed') {
    return (
      <Card variant="surface" role="alert">
        <CardHeader>
          <div className="app-result__heading">
            <CardTitle>Não foi possível consultar</CardTitle>
            {/*
              O badge diz "falha na consulta" e não "erro de rede": o mesmo
              caminho de rejeição transporta causas que não são de rede — o
              plugin responde `AssetNotFound` ("no update asset matched target
              …") quando o manifesto não tem asset para esta plataforma, e isso
              não é um problema de rede. A causa vem em texto e é mostrada
              abaixo, como veio. Com os relays em baixo — o estado de hoje — a
              mensagem é de rede, e é o caso que este cartão mostra na prática.
            */}
            <Badge status="error">falha na consulta</Badge>
          </div>
          <CardDescription>
            Isto <strong>não</strong> é "não há novidade": a consulta não chegou a produzir um
            veredito. A causa está abaixo, no texto que o plugin devolveu — hoje é a resolução da
            árvore de releases, que nenhum dos relays responde.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <pre className="app-failure__detail">{result.message}</pre>
        </CardContent>
      </Card>
    );
  }

  if (result.kind === 'unavailable') {
    return (
      <Card variant="surface" role="status">
        <CardHeader>
          <div className="app-result__heading">
            <CardTitle>Consulta indisponível</CardTitle>
            <Badge status="pending">fora do Tauri</Badge>
          </div>
          <CardDescription>
            O pedido foi feito sem runtime Tauri, portanto não houve processo nativo que pudesse
            falar com os relays. Isto não é um erro de rede nem um "não há novidade" — é uma
            consulta que <strong>não foi feita</strong>. A verificação de atestações acima continua
            a funcionar: é WASM.
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  if (result.kind === 'no-release') {
    return (
      <Card variant="surface" role="status">
        <CardHeader>
          <div className="app-result__heading">
            <CardTitle>Nada publicado</CardTitle>
            <Badge status="pending">sem release</Badge>
          </div>
          <CardDescription>
            O plugin não encontrou release nem manifesto no caminho declarado — é o que responde
            quando ainda não se publicou nada. Deliberadamente distinto de "já actualizado": aqui
            não houve comparação de versões nenhuma, e portanto não há nada a afirmar sobre estar
            ou não em dia. (Um manifesto que exista mas não tenha asset para esta plataforma{' '}
            <strong>não</strong> cai aqui: chega como falha, com a causa em texto.)
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  const available = result.kind === 'available';

  return (
    <Card role="status" aria-live="polite">
      <CardHeader>
        <div className="app-result__heading">
          <CardTitle>{available ? 'Actualização disponível' : 'Sem novidade'}</CardTitle>
          {/*
            `partial` para "há actualização" não é um veredito de verificação — é
            o estado de atenção da paleta. O texto é que carrega o significado
            (WCAG 2.2 SC 1.4.1), como nos outros badges.
          */}
          <Badge status={available ? 'partial' : 'verified'}>
            {available ? `versão ${result.update.version}` : 'já actualizado'}
          </Badge>
        </div>
        <CardDescription>{updateSummary(result.update, available)}</CardDescription>
      </CardHeader>
      <CardContent>
        {/* A mesma lista termo/valor da inspecção: aqui os valores longos são
            nomes de asset, que têm de poder quebrar em vez de empurrar o termo
            para fora do cartão. */}
        <dl className="app-inspect__facts">
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Versão em execução</dt>
            <dd className="app-inspect__fact-value">{result.update.currentVersion}</dd>
          </div>
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Versão publicada</dt>
            <dd className="app-inspect__fact-value">{result.update.version}</dd>
          </div>
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Asset seleccionado</dt>
            <dd className="app-inspect__fact-value">{result.update.assetName}</dd>
          </div>
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Tipo</dt>
            <dd className="app-inspect__fact-value">{result.update.assetKind}</dd>
          </div>
          <div className="app-inspect__fact">
            <dt className="app-inspect__fact-term">Publicado em</dt>
            <dd className="app-inspect__fact-value">
              {result.update.publishedAt ?? 'não declarado'}
            </dd>
          </div>
        </dl>
      </CardContent>
      {result.update.notes !== null && (
        <CardFooter>
          <p className="app-result__cause">
            <span className="app-result__cause-term">Notas: </span>
            {result.update.notes}
          </p>
        </CardFooter>
      )}
    </Card>
  );
}

/**
 * A frase de estado, com os números dentro.
 *
 * "Não é mais recente" e não "é mais antiga": as duas versões podem ser iguais
 * (o caso normal) ou a publicada pode ser anterior — um rollback. Uma frase que
 * só admitisse a igualdade estaria a mentir num dos dois casos.
 */
function updateSummary(update: UpdateInfo, available: boolean): string {
  return available
    ? `Corre a ${update.currentVersion}; está publicado ${update.version}, que é mais recente.`
    : `Corre a ${update.currentVersion}; a versão publicada (${update.version}) não é mais recente.`;
}
