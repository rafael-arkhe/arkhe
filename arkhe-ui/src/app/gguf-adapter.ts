/**
 * A fronteira com o **núcleo nativo** — o único módulo da app que sabe que
 * existe um processo Rust do outro lado, e o único que sabe o nome do comando.
 * O resto da app fala só com `inspectGgufModel`.
 *
 * # Porque é que isto **não** vive no `verify-adapter.ts`
 *
 * Os dois são fronteiras de verificação, e são fronteiras **diferentes** em
 * quatro eixos — juntá-las obrigaria a que uma delas mentisse sobre o contrato:
 *
 * 1. **Disponibilidade.** O `verify-adapter` funciona sempre: é WASM, corre
 *    tanto no navegador como dentro do Tauri. Este só funciona dentro do Tauri.
 *    O cabeçalho do `verify-adapter` diz que é "o único módulo da app que sabe
 *    que existe um WebAssembly"; meter-lhe dentro conhecimento de um *runtime
 *    que pode não existir* tornaria essa frase falsa.
 * 2. **A entrada é outra coisa.** Ali entram duas *strings* JSON que já estão em
 *    memória. Aqui entra um **caminho**, e quem lê o ficheiro é o processo Rust
 *    — os bytes nunca passam pelo heap do webview. É deliberado: um modelo de
 *    vários GB não deve atravessar o IPC.
 * 3. **Outro ponto de entrada do core.** Ali é `verify_attestation` (a fachada,
 *    o pipeline de quatro estágios). Aqui é `arkhe_verify::gguf::{model_digest,
 *    parse_header}` — inspecção de um modelo, que não é um veredito sobre uma
 *    atestação. Não se implicam: um modelo pode ter digest calculável e
 *    cabeçalho truncado.
 * 4. **Escada de degradação diferente.** Ali, "sem WASM" é uma falha a reportar.
 *    Aqui, "sem Tauri" é o **estado normal** quando a app corre no navegador —
 *    um `dist-app` servido estaticamente é um alvo suportado, não um erro.
 *
 * O que os dois partilham é só o idioma: resultado como união discriminada, em
 * vez de `null`s que se confundem. Ver `WasmState` em `App.tsx`.
 *
 * # O que este módulo **não** faz
 *
 * Não abre diálogo de ficheiros: não há plugin de diálogo nem permissão de
 * filesystem configurados, portanto o caminho é escrito pelo utilizador. Não
 * valida o caminho — o comando lê qualquer caminho que lhe seja dado, com os
 * privilégios do utilizador; a nota de segurança em `src-tauri/src/lib.rs`
 * explica quando é que isso deixa de ser aceitável.
 */
import { invoke, isTauri } from '@tauri-apps/api/core';

/** O nome do comando, registado em `src-tauri/src/lib.rs`. */
const INSPECT_COMMAND = 'inspect_gguf_model';

/**
 * O cabeçalho lido dos bytes do modelo — espelho de
 * `arkhe_verify::gguf::GgufHeaderReport`.
 *
 * Os três campos anuláveis são `Option<T>` do lado do Rust: `null` **não** é
 * "zero", é "estes bytes não estavam presentes no ficheiro". Uma versão 0 e uma
 * versão ausente são factos diferentes, e é essa a distinção que a inspecção
 * existe para mostrar.
 */
export interface GgufHeaderReport {
  /** Só é `true` se o cabeçalho inteiro é válido. */
  ok: boolean;
  /** `true` se os quatro primeiros bytes são `GGUF`. */
  magic_ok: boolean;
  /** Quantos bytes foram entregues ao parser — a aritmética do truncamento. */
  available_bytes: number;
  version: number | null;
  tensor_count: number | null;
  metadata_kv_count: number | null;
  /** A causa da recusa, ou `null` se passou. */
  error: string | null;
}

/** Espelho de `ModelInspection` em `src-tauri/src/lib.rs`. */
export interface ModelInspection {
  /** O caminho lido, ecoado de volta pelo comando. */
  path: string;
  bytes: number;
  /** `SHA-256(model)`, em hex minúsculo. */
  digest_hex: string;
  header: GgufHeaderReport;
}

/**
 * O que pode acontecer ao pedir uma inspecção — três casos, não dois.
 *
 * `unavailable` existe separado de `failed` de propósito. "Não há processo
 * nativo" não é "o nativo falhou": o primeiro é o comportamento correcto de um
 * `dist-app` servido no navegador, o segundo é um problema. Colapsá-los num
 * `failed` faria a app reportar um erro onde não há erro nenhum.
 */
export type InspectionResult =
  | { kind: 'inspected'; inspection: ModelInspection }
  | { kind: 'unavailable' }
  | { kind: 'failed'; message: string };

/**
 * Há um runtime Tauri a atender o IPC?
 *
 * Delega no `isTauri()` do `@tauri-apps/api`, que lê `globalThis.isTauri` — a
 * flag que o Tauri v2 injecta no webview. É deliberadamente uma leitura em tempo
 * de chamada e não uma constante de módulo: um valor capturado no import seria
 * uma resposta a uma pergunta feita antes de a página estar montada.
 */
export function nativeInspectionAvailable(): boolean {
  return isTauri();
}

/** `null` para ausente, número finito para presente, e nada mais. */
function numberOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

/**
 * Estreita a resposta do IPC para a forma declarada — ou recusa-a.
 *
 * Aqui divergimos do [`parseReport`](./verify-adapter.ts) de propósito, e a
 * razão é o que está em jogo. Um relatório de atestação tem um valor
 * **conservador** para onde cair: um campo que não se interpreta vira `false`,
 * que significa "não verificado" — o lado seguro.
 *
 * Uma inspecção de modelo não tem lado seguro. Não há um valor para `bytes` ou
 * para `digest_hex` que signifique "não sei": inventar `0` ou uma string vazia
 * seria apresentar um facto falso como se fosse medido. Por isso esta função
 * devolve `null` em vez de inventar — e quem chama reporta "forma inesperada".
 */
export function parseInspection(value: unknown): ModelInspection | null {
  if (typeof value !== 'object' || value === null) return null;

  const raw = value as Record<string, unknown>;
  const header = raw.header;
  if (typeof header !== 'object' || header === null) return null;
  const rawHeader = header as Record<string, unknown>;

  const path = typeof raw.path === 'string' ? raw.path : null;
  const bytes = numberOrNull(raw.bytes);
  const digestHex = typeof raw.digest_hex === 'string' ? raw.digest_hex : null;

  const ok = typeof rawHeader.ok === 'boolean' ? rawHeader.ok : null;
  const magicOk = typeof rawHeader.magic_ok === 'boolean' ? rawHeader.magic_ok : null;
  const availableBytes = numberOrNull(rawHeader.available_bytes);

  if (
    path === null ||
    bytes === null ||
    digestHex === null ||
    ok === null ||
    magicOk === null ||
    availableBytes === null
  ) {
    return null;
  }

  // `error` é `Option<String>`: ausente é `null` legítimo, mas um `error` que
  // esteja presente e não seja string é uma forma que não se interpreta — e
  // engolir a causa de uma recusa seria pior do que admitir que não se sabe ler.
  const rawError = rawHeader.error;
  if (rawError !== null && rawError !== undefined && typeof rawError !== 'string') {
    return null;
  }

  return {
    path,
    bytes,
    digest_hex: digestHex,
    header: {
      ok,
      magic_ok: magicOk,
      available_bytes: availableBytes,
      version: numberOrNull(rawHeader.version),
      tensor_count: numberOrNull(rawHeader.tensor_count),
      metadata_kv_count: numberOrNull(rawHeader.metadata_kv_count),
      error: typeof rawError === 'string' ? rawError : null,
    },
  };
}

/**
 * Pede ao processo Rust que leia um modelo do disco e o inspeccione.
 *
 * **Nunca lança.** As três saídas estão em [`InspectionResult`], para que o
 * chamador tenha de olhar para as três em vez de apanhar uma excepção genérica.
 *
 * Fora do Tauri devolve `unavailable` **sem chamar o `invoke`** — o `invoke`
 * desreferencia `window.__TAURI_INTERNALS__` e lançaria. A guarda não é
 * defensiva a mais: é o caso normal de um `dist-app` servido no navegador.
 */
export async function inspectGgufModel(path: string): Promise<InspectionResult> {
  if (!isTauri()) return { kind: 'unavailable' };

  let raw: unknown;
  try {
    raw = await invoke<unknown>(INSPECT_COMMAND, { path });
  } catch (error: unknown) {
    // O `Err(String)` do Rust chega aqui como rejeição com a string dentro.
    return { kind: 'failed', message: error instanceof Error ? error.message : String(error) };
  }

  const inspection = parseInspection(raw);
  if (inspection === null) {
    return {
      kind: 'failed',
      message:
        'a resposta de `inspect_gguf_model` não tem a forma declarada — não é interpretada em vez de adivinhada',
    };
  }

  return { kind: 'inspected', inspection };
}
