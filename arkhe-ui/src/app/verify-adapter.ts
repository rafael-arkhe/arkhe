/**
 * A fronteira com o WASM — **o único módulo da app que sabe que existe um
 * WebAssembly** (`crates/arkhe-verify-wasm`, alvo `web`) e o único que sabe
 * onde ele vive no disco. O resto da app fala só com `VerifyApi`.
 *
 * O artefacto consumido é o `pkg-web` (wasm-bindgen `--target web`), não o
 * `pkg/` (`--target nodejs`): o glue `web` é o que um navegador carrega — tem
 * `init`/`initSync` e resolve o `.wasm` por `import.meta.url`, que é o que faz
 * o Vite emitir o binário como asset no build de app.
 *
 * Como (re)gerar o artefacto, a partir da raiz de `safe-core-monorepo`:
 *
 *     cargo build --release --target wasm32-unknown-unknown -p arkhe-verify-wasm
 *     wasm-bindgen --target web --out-dir crates/arkhe-verify-wasm/pkg-web \
 *       target/wasm32-unknown-unknown/release/arkhe_verify_wasm.wasm
 *
 * (`pkg-web/` é artefacto de build e não entra no git.)
 */
import initVerifyGlue, {
  initSync,
  verify_attestation,
} from '../../../safe-core-monorepo/crates/arkhe-verify-wasm/pkg-web/arkhe_verify_wasm.js';

/** O relatório que `verify_attestation` devolve, já interpretado. */
export interface AttestationReport {
  ok: boolean;
  sha256: boolean;
  signature: boolean;
  inclusion: boolean;
  quorum: boolean;
  error: string | null;
}

/** O que a app pode pedir ao verificador. */
export interface VerifyApi {
  /**
   * Confere uma atestação completa contra um trust root.
   *
   * Não lança sobre entradas não confiáveis: uma atestação (ou um trust root)
   * malformada devolve um relatório com `ok: false` e a causa em `error`.
   */
  verifyAttestation(attestationJson: string, trustRootJson: string): AttestationReport;
}

/**
 * Interpreta o relatório JSON.
 *
 * O crate garante a forma do relatório, mas isto é uma fronteira com um
 * módulo compilado: os campos são estreitados em vez de assumidos, para que
 * uma divergência apareça como campo `false` — e não como um objecto com
 * campos `undefined` a propagar-se pela UI.
 */
export function parseReport(reportJson: string): AttestationReport {
  const value = JSON.parse(reportJson) as Partial<Record<keyof AttestationReport, unknown>>;
  return {
    ok: value.ok === true,
    sha256: value.sha256 === true,
    signature: value.signature === true,
    inclusion: value.inclusion === true,
    quorum: value.quorum === true,
    error: typeof value.error === 'string' ? value.error : null,
  };
}

function api(): VerifyApi {
  return {
    verifyAttestation: (attestationJson, trustRootJson) =>
      parseReport(verify_attestation(attestationJson, trustRootJson)),
  };
}

/**
 * Inicializa o verificador a partir dos bytes do módulo.
 *
 * Síncrono e sem `fetch`: é o caminho que os testes usam (o `.wasm` lido do
 * disco) e o que um embed sem bundler usaria. Passa pelo **mesmo** glue e
 * devolve o **mesmo** `VerifyApi` que o [`loadVerify`] — não é um mock nem
 * uma segunda implementação.
 */
export function initVerifyFromBytes(bytes: BufferSource | WebAssembly.Module): VerifyApi {
  initSync({ module: bytes });
  return api();
}

/**
 * Inicializa o verificador pelo caminho do navegador: o `init` assíncrono do
 * glue, que resolve o `.wasm` a partir de `import.meta.url` — o Vite
 * transforma isso num asset do build. É esta a entrada que a app usa.
 */
export async function loadVerify(): Promise<VerifyApi> {
  await initVerifyGlue();
  return api();
}
